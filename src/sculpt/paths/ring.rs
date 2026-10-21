//! Mesh section loops and winding-constrained surface routes. Both start on
//! the touched sheet and return a private, closed path without a repeated seam.
use super::{MAX_POINTS, PathPoint};
use crate::sculpt::{Mesh, Vec3};
use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap, HashSet},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Key {
    Vertex(usize),
    Edge(usize, usize),
}
struct Node {
    key: Key,
    point: PathPoint,
    neighbors: Vec<usize>,
}

fn finite(p: Vec3) -> bool {
    p.0.is_finite() && p.1.is_finite() && p.2.is_finite()
}

fn inputs(mesh: &Mesh, hit: PathPoint, axis: Vec3) -> Result<(Vec3, f32), String> {
    if !finite(hit.position)
        || !finite(hit.normal)
        || hit.normal.len() < 0.5
        || !finite(axis)
        || axis.len() < 1e-6
        || mesh.triangles.is_empty()
    {
        return Err("Touch a visible surface and choose a valid loop axis".into());
    }
    if mesh
        .triangles
        .iter()
        .any(|face| face.iter().any(|&index| index >= mesh.vertices.len()))
    {
        return Err("The loop target contains an invalid triangle".into());
    }
    let scale = mesh
        .vertices
        .iter()
        .map(|&p| (p - hit.position).len())
        .fold(0_f32, f32::max);
    if !scale.is_finite() || scale < 1e-6 || mesh.vertices.iter().any(|&p| !finite(p)) {
        return Err("The loop target needs finite surface geometry".into());
    }
    Ok((axis.norm(), (scale * 1e-6).max(1e-7)))
}

fn nearest_on_segment(hit: Vec3, a: PathPoint, b: PathPoint) -> (f32, f32) {
    let edge = b.position - a.position;
    let t = ((hit - a.position).dot(edge) / edge.dot(edge).max(f32::MIN_POSITIVE)).clamp(0., 1.);
    ((a.position + edge * t - hit).len(), t)
}

fn interpolate(a: PathPoint, b: PathPoint, t: f32) -> PathPoint {
    PathPoint {
        position: a.position * (1. - t) + b.position * t,
        normal: (a.normal * (1. - t) + b.normal * t).norm(),
    }
}

fn vertex_node(
    key: Key,
    point: PathPoint,
    nodes: &mut Vec<Node>,
    lookup: &mut HashMap<Key, usize>,
) -> usize {
    *lookup.entry(key).or_insert_with(|| {
        let index = nodes.len();
        nodes.push(Node {
            key,
            point,
            neighbors: Vec::new(),
        });
        index
    })
}

/// The level ring is the connected visible mesh/plane section through the hit,
/// perpendicular to the explicit axis. Topological identities prevent a jump
/// between nearby shells. Coplanar caps, branches, and open cuts are rejected.
pub fn section_ring(mesh: &Mesh, hit: PathPoint, axis: Vec3) -> Result<Vec<PathPoint>, String> {
    let (axis, epsilon) = inputs(mesh, hit, axis)?;
    let normals = mesh.normals();
    let distances: Vec<_> = mesh
        .vertices
        .iter()
        .map(|&p| (p - hit.position).dot(axis))
        .collect();
    let mut nodes = Vec::<Node>::new();
    let mut lookup = HashMap::new();
    let mut edges = HashMap::<[usize; 2], usize>::new();
    let mut coplanar = HashSet::new();
    for (face, &triangle) in mesh.triangles.iter().enumerate() {
        if !mesh.face_visible(face) {
            continue;
        }
        if triangle.iter().any(|&v| v >= mesh.vertices.len()) {
            return Err("Invalid target triangle".into());
        }
        let zeros = triangle
            .iter()
            .filter(|&&v| distances[v].abs() <= epsilon)
            .count();
        if zeros == 3 {
            coplanar.extend(triangle);
            continue;
        }
        let mut cut = Vec::with_capacity(3);
        for &v in &triangle {
            if distances[v].abs() <= epsilon {
                cut.push(vertex_node(
                    Key::Vertex(v),
                    PathPoint {
                        position: mesh.vertices[v],
                        normal: normals[v],
                    },
                    &mut nodes,
                    &mut lookup,
                ));
            }
        }
        for (a, b) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            let da = distances[a];
            let db = distances[b];
            if da.abs() <= epsilon || db.abs() <= epsilon || da.signum() == db.signum() {
                continue;
            }
            let key = Key::Edge(a.min(b), a.max(b));
            // Always interpolate in key order so neighboring faces agree exactly.
            let (a, b) = (a.min(b), a.max(b));
            let t = distances[a] / (distances[a] - distances[b]);
            let point = interpolate(
                PathPoint {
                    position: mesh.vertices[a],
                    normal: normals[a],
                },
                PathPoint {
                    position: mesh.vertices[b],
                    normal: normals[b],
                },
                t,
            );
            cut.push(vertex_node(key, point, &mut nodes, &mut lookup));
        }
        cut.sort_unstable();
        cut.dedup();
        if cut.len() == 2 {
            let edge = [cut[0], cut[1]];
            let count = edges.entry(edge).or_default();
            *count += 1;
            if *count == 1 {
                nodes[edge[0]].neighbors.push(edge[1]);
                nodes[edge[1]].neighbors.push(edge[0]);
            }
        } else if cut.len() > 2 {
            return Err(
                "The section is singular; move the touch point away from a flat face".into(),
            );
        }
    }
    if edges.is_empty() {
        return Err("No closed surface section passes through this touch point".into());
    }
    let mut component = vec![usize::MAX; nodes.len()];
    let mut members = Vec::<Vec<usize>>::new();
    for seed in 0..nodes.len() {
        if component[seed] != usize::MAX || nodes[seed].neighbors.is_empty() {
            continue;
        }
        let id = members.len();
        let mut group = vec![seed];
        component[seed] = id;
        let mut next = 0;
        while next < group.len() {
            for &other in &nodes[group[next]].neighbors {
                if component[other] == usize::MAX {
                    component[other] = id;
                    group.push(other);
                }
            }
            next += 1;
        }
        members.push(group);
    }
    let mut nearest = vec![f32::INFINITY; members.len()];
    for &[a, b] in edges.keys() {
        nearest[component[a]] = nearest[component[a]]
            .min(nearest_on_segment(hit.position, nodes[a].point, nodes[b].point).0);
    }
    let selected = (0..nearest.len())
        .min_by(|&a, &b| nearest[a].total_cmp(&nearest[b]))
        .ok_or("No surface contour found")?;
    let tolerance = epsilon * 32.;
    if nearest[selected] > tolerance {
        return Err(
            "The touched surface has no closed section here; move away from a tip or boundary"
                .into(),
        );
    }
    if nearest
        .iter()
        .enumerate()
        .any(|(id, &distance)| id != selected && distance <= nearest[selected] + tolerance)
    {
        return Err("The section is ambiguous between touching or overlapping shells".into());
    }
    if members[selected]
        .iter()
        .any(|&i| nodes[i].neighbors.len() != 2)
    {
        return Err(
            "The section is open or branches; show the whole surface and choose another height"
                .into(),
        );
    }
    if members[selected]
        .iter()
        .any(|&i| matches!(nodes[i].key,Key::Vertex(v) if coplanar.contains(&v)))
        || edges
            .iter()
            .any(|(edge, &count)| component[edge[0]] == selected && count > 2)
    {
        return Err("The section is singular at a flat cap or non-manifold junction; slide to another height".into());
    }
    let start = members[selected][0];
    let mut ordered = Vec::with_capacity(members[selected].len());
    let (mut previous, mut current) = (usize::MAX, start);
    loop {
        ordered.push(nodes[current].point);
        let next = nodes[current]
            .neighbors
            .iter()
            .copied()
            .find(|&n| n != previous)
            .unwrap();
        previous = current;
        current = next;
        if current == start {
            break;
        }
        if ordered.len() > members[selected].len() {
            return Err("The section cannot form one simple closed loop".into());
        }
    }
    finish_ring(ordered, hit, axis, epsilon)
}

fn finish_ring(
    mut points: Vec<PathPoint>,
    hit: PathPoint,
    axis: Vec3,
    epsilon: f32,
) -> Result<Vec<PathPoint>, String> {
    if points.len() < 3
        || points
            .iter()
            .any(|p| !finite(p.position) || !finite(p.normal) || p.normal.len() < 0.5)
    {
        return Err("The surface loop has too few usable points or ambiguous normals".into());
    }
    let area: f32 = (0..points.len())
        .map(|i| {
            (points[i].position - hit.position)
                .cross(points[(i + 1) % points.len()].position - hit.position)
                .dot(axis)
        })
        .sum();
    if !area.is_finite() || area.abs() < epsilon * epsilon {
        return Err("The section collapses instead of surrounding the body".into());
    }
    if area < 0. {
        points.reverse();
    }
    let (segment, (_, t)) = (0..points.len())
        .map(|i| {
            (
                i,
                nearest_on_segment(hit.position, points[i], points[(i + 1) % points.len()]),
            )
        })
        .min_by(|a, b| a.1.0.total_cmp(&b.1.0))
        .unwrap();
    let seam = interpolate(points[segment], points[(segment + 1) % points.len()], t);
    if seam.normal.dot(hit.normal.norm()) < 0.15 {
        return Err("The section reaches an opposing sheet instead of the touched surface".into());
    }
    let seam_index = if t < 1e-5 {
        segment
    } else if t > 1. - 1e-5 {
        (segment + 1) % points.len()
    } else {
        points.insert(segment + 1, seam);
        segment + 1
    };
    points.rotate_left(seam_index);
    if points.len() > MAX_POINTS {
        let source = points;
        let mut lengths = vec![0.];
        for i in 0..source.len() {
            lengths.push(
                lengths.last().unwrap()
                    + (source[(i + 1) % source.len()].position - source[i].position).len(),
            );
        }
        let total = *lengths.last().unwrap();
        let mut edge = 0;
        points = Vec::with_capacity(MAX_POINTS);
        for i in 0..MAX_POINTS {
            let distance = total * i as f32 / MAX_POINTS as f32;
            while edge + 2 < lengths.len() && lengths[edge + 1] <= distance {
                edge += 1;
            }
            let t = (distance - lengths[edge])
                / (lengths[edge + 1] - lengths[edge]).max(f32::MIN_POSITIVE);
            points.push(interpolate(
                source[edge],
                source[(edge + 1) % source.len()],
                t,
            ));
        }
    }
    for point in &mut points {
        point.normal = point.normal.norm();
    }
    Ok(points)
}

struct RouteNode {
    point: PathPoint,
    angle: f32,
    radial: Vec3,
    neighbors: Vec<(usize, f32, i8)>,
}
#[derive(Clone, Copy)]
struct QueueEntry {
    distance: f32,
    state: usize,
}
impl PartialEq for QueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance && self.state == other.state
    }
}
impl Eq for QueueEntry {}
impl PartialOrd for QueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for QueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .distance
            .total_cmp(&self.distance)
            .then_with(|| other.state.cmp(&self.state))
    }
}

fn connect(
    nodes: &mut [RouteNode],
    seen: &mut HashSet<[usize; 2]>,
    a: usize,
    b: usize,
    min_radius: f32,
) {
    if a == b || !seen.insert([a.min(b), a.max(b)]) {
        return;
    }
    let edge = nodes[b].radial - nodes[a].radial;
    let t = (-nodes[a].radial.dot(edge) / edge.dot(edge).max(f32::MIN_POSITIVE)).clamp(0., 1.);
    if (nodes[a].radial + edge * t).len() < min_radius {
        return;
    }
    let raw = nodes[b].angle - nodes[a].angle;
    let jump = if raw < -std::f32::consts::PI {
        1
    } else if raw > std::f32::consts::PI {
        -1
    } else {
        0
    };
    let length = (nodes[b].point.position - nodes[a].point.position).len();
    nodes[a].neighbors.push((b, length, jump));
    nodes[b].neighbors.push((a, length, -jump));
}

/// Shortest forward-winding route on the visible side-surface graph. Graph edges are
/// actual mesh edges plus the level section within each cut triangle, so this
/// is a mesh-resolution-dependent approximation, not an exact smooth geodesic.
/// The axis line passes through the selected level section's projected bounds
/// center. Cap shortcuts and edges through that line are excluded.
pub fn shortest_ring(mesh: &Mesh, hit: PathPoint, axis: Vec3) -> Result<Vec<PathPoint>, String> {
    shortest_ring_cancelled(mesh, hit, axis, &|| false)
}

pub fn shortest_ring_cancelled(
    mesh: &Mesh,
    hit: PathPoint,
    axis: Vec3,
    cancel: &impl Fn() -> bool,
) -> Result<Vec<PathPoint>, String> {
    if cancel() {
        return Err("Loop construction cancelled".into());
    }
    let (axis, epsilon) = inputs(mesh, hit, axis)?;
    let section = section_ring(mesh, hit, axis)?;
    let u = axis
        .cross(if axis.0.abs() < 0.8 {
            Vec3(1., 0., 0.)
        } else {
            Vec3(0., 1., 0.)
        })
        .norm();
    let v = axis.cross(u);
    let (mut min_u, mut max_u, mut min_v, mut max_v) = (
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
    );
    for p in &section {
        let relative = p.position - hit.position;
        min_u = min_u.min(relative.dot(u));
        max_u = max_u.max(relative.dot(u));
        min_v = min_v.min(relative.dot(v));
        max_v = max_v.max(relative.dot(v));
    }
    let center = hit.position + u * ((min_u + max_u) * 0.5) + v * ((min_v + max_v) * 0.5);
    let radial = |position: Vec3| {
        let d = position - center;
        d - axis * d.dot(axis)
    };
    let min_radius = section
        .iter()
        .map(|p| radial(p.position).len())
        .fold(f32::INFINITY, f32::min)
        * 0.02;
    let min_radius = min_radius.max(epsilon * 32.);
    let turn: f32 = (0..section.len())
        .map(|i| {
            let a = radial(section[i].position);
            let b = radial(section[(i + 1) % section.len()].position);
            axis.dot(a.cross(b)).atan2(a.dot(b))
        })
        .sum();
    if (turn.abs() - std::f32::consts::TAU).abs() > 0.05 {
        return Err("This section does not surround the chosen axis once; choose another axis or level loop".into());
    }
    let seed_radial = radial(hit.position).norm();
    let side = axis.cross(seed_radial);
    let make_node = |point: PathPoint| {
        let r = radial(point.position);
        RouteNode {
            point,
            angle: r.dot(side).atan2(r.dot(seed_radial)),
            radial: r,
            neighbors: Vec::new(),
        }
    };
    let normals = mesh.normals();
    let mut nodes: Vec<_> = mesh
        .vertices
        .iter()
        .enumerate()
        .map(|(i, &position)| {
            make_node(PathPoint {
                position,
                normal: normals[i],
            })
        })
        .collect();
    let anchor = nodes.len();
    nodes.push(make_node(hit));
    let mut seen = HashSet::new();
    let mut edge_cuts = HashMap::<[usize; 2], usize>::new();
    let signed: Vec<_> = mesh
        .vertices
        .iter()
        .map(|&p| (p - hit.position).dot(axis))
        .collect();
    let mut hit_faces = 0;
    for (face, &triangle) in mesh.triangles.iter().enumerate() {
        if face % 1024 == 0 && cancel() {
            return Err("Loop construction cancelled".into());
        }
        if !mesh.face_visible(face) {
            continue;
        }
        let [a, b, c] = triangle.map(|i| mesh.vertices[i]);
        let normal = (b - a).cross(c - a).norm();
        // A flat cap provides a shortcut through the body, not circumnavigation.
        if normal.dot(axis).abs() > 0.97 {
            continue;
        }
        let mut cut = Vec::with_capacity(3);
        for &vertex in &triangle {
            if signed[vertex].abs() <= epsilon {
                cut.push(vertex);
            }
        }
        for (a, b) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            connect(&mut nodes, &mut seen, a, b, min_radius);
            if signed[a].abs() <= epsilon
                || signed[b].abs() <= epsilon
                || signed[a].signum() == signed[b].signum()
            {
                continue;
            }
            let key = [a.min(b), a.max(b)];
            let crossing = *edge_cuts.entry(key).or_insert_with(|| {
                let [a, b] = key;
                let t = signed[a] / (signed[a] - signed[b]);
                let index = nodes.len();
                nodes.push(make_node(interpolate(nodes[a].point, nodes[b].point, t)));
                index
            });
            connect(&mut nodes, &mut seen, a, crossing, min_radius);
            connect(&mut nodes, &mut seen, b, crossing, min_radius);
            cut.push(crossing);
        }
        cut.sort_unstable();
        cut.dedup();
        if cut.len() == 2 {
            connect(&mut nodes, &mut seen, cut[0], cut[1], min_radius);
        }
        let closest = crate::sculpt::math::closest_triangle(hit.position, a, b, c);
        if (closest - hit.position).len() <= epsilon * 8. && normal.dot(hit.normal) > 0.15 {
            hit_faces += 1;
            for node in triangle.into_iter().chain(cut) {
                connect(&mut nodes, &mut seen, anchor, node, min_radius);
            }
        }
        if nodes.len() > 750_000 || seen.len() > 2_500_000 {
            return Err("The shortest-loop graph exceeds the interactive work limit; use a level loop or a coarser surface".into());
        }
    }
    if hit_faces == 0 || nodes[anchor].neighbors.is_empty() {
        return Err("Touch the side of the body to seed a shortest surface loop".into());
    }
    let perimeter: f32 = (0..section.len())
        .map(|i| (section[(i + 1) % section.len()].position - section[i].position).len())
        .sum();
    let ceiling = perimeter * 1.01 + epsilon * 32.;
    let states = nodes.len() * 3;
    let start = anchor * 3 + 1;
    let mut distances = vec![f32::INFINITY; states];
    let mut previous = vec![usize::MAX; states];
    distances[start] = 0.;
    let mut queue = BinaryHeap::from([QueueEntry {
        distance: 0.,
        state: start,
    }]);
    let mut work = 0usize;
    let mut end = None;
    while let Some(entry) = queue.pop() {
        if entry.distance > distances[entry.state] {
            continue;
        }
        if work.is_multiple_of(1024) && cancel() {
            return Err("Loop construction cancelled".into());
        }
        let node = entry.state / 3;
        let sheet = (entry.state % 3) as i8;
        if node == anchor && sheet != 1 {
            end = Some(entry.state);
            break;
        }
        for &(neighbor, length, jump) in &nodes[node].neighbors {
            work += 1;
            if work > 8_000_000 {
                return Err(
                    "The shortest-loop search exceeds the interactive work limit; use a level loop"
                        .into(),
                );
            }
            let sheet = sheet + jump;
            if !(0..=2).contains(&sheet) {
                continue;
            }
            let angle_step =
                nodes[neighbor].angle - nodes[node].angle + std::f32::consts::TAU * jump as f32;
            let progress = nodes[neighbor].angle + std::f32::consts::TAU * (sheet - 1) as f32;
            // A circumnavigation advances around the chosen axis exactly once.
            // This excludes winding loops with a shared out-and-back stem and
            // keeps the touched point on the ring instead of on a lollipop tail.
            if angle_step < -1e-6
                || !(-1e-6..=std::f32::consts::TAU + 1e-6).contains(&progress)
                || (angle_step.abs() < 1e-6
                    && length > epsilon
                    && (progress.abs() < 1e-6 || (progress - std::f32::consts::TAU).abs() < 1e-6))
            {
                continue;
            }
            let state = neighbor * 3 + sheet as usize;
            let distance = entry.distance + length;
            if distance < distances[state] && distance <= ceiling {
                distances[state] = distance;
                previous[state] = entry.state;
                queue.push(QueueEntry { distance, state });
            }
        }
    }
    let mut state = end.ok_or(
        "No unambiguous one-turn side-surface route surrounds this axis; use a level loop",
    )?;
    let mut points = Vec::new();
    while state != start {
        points.push(nodes[state / 3].point);
        state = previous[state];
        if state == usize::MAX || points.len() > states {
            return Err("The shortest-loop route is disconnected".into());
        }
    }
    points.push(hit);
    points.reverse();
    points.pop();
    // The seam can coincide with a mesh vertex, creating a zero-cost anchor edge.
    points.dedup_by(|a, b| (a.position - b.position).len() < epsilon);
    if points.len() > 1 && (points.last().unwrap().position - points[0].position).len() < epsilon {
        points.pop();
    }
    finish_ring(points, hit, axis, epsilon)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{
        Primitive, PrimitiveKind,
        paths::{PathMode, SurfacePath},
    };
    const SEGMENTS: usize = 48;

    fn cylinder(taper: f32, bend: f32) -> Mesh {
        let mut primitive = Primitive::for_kind(PrimitiveKind::Cylinder);
        primitive.segments = SEGMENTS as u32;
        primitive.divisions = [8; 3];
        let mut mesh = primitive.mesh().unwrap();
        for p in &mut mesh.vertices {
            let radius = 1. - taper * p.1;
            p.0 = p.0 * radius + bend * p.1 * p.1;
            p.2 *= radius;
        }
        mesh
    }
    fn hit(mesh: &Mesh, height: f32) -> PathPoint {
        let row = ((height + 1.) * 4.).floor().clamp(0., 7.) as usize;
        let t = ((height + 1.) * 4. - row as f32).clamp(0., 1.);
        let normals = mesh.normals();
        interpolate(
            PathPoint {
                position: mesh.vertices[row * SEGMENTS],
                normal: normals[row * SEGMENTS],
            },
            PathPoint {
                position: mesh.vertices[(row + 1) * SEGMENTS],
                normal: normals[(row + 1) * SEGMENTS],
            },
            t,
        )
    }
    fn perimeter(points: &[PathPoint]) -> f32 {
        (0..points.len())
            .map(|i| (points[(i + 1) % points.len()].position - points[i].position).len())
            .sum()
    }
    fn check_loop(mesh: &Mesh, points: &[PathPoint], hit: PathPoint) {
        assert!((3..=MAX_POINTS).contains(&points.len()));
        assert!((points[0].position - hit.position).len() < 1e-4);
        assert!((points[0].position - points.last().unwrap().position).len() > 1e-6);
        for point in points {
            assert!((point.normal.len() - 1.).abs() < 1e-5);
            let distance = mesh
                .triangles
                .iter()
                .map(|face| {
                    let [a, b, c] = face.map(|i| mesh.vertices[i]);
                    (crate::sculpt::math::closest_triangle(point.position, a, b, c)
                        - point.position)
                        .len()
                })
                .fold(f32::INFINITY, f32::min);
            assert!(distance < 1e-4, "loop point left surface by {distance}");
        }
        let path = SurfacePath {
            id: 1,
            target: 1,
            name: "Ring".into(),
            mode: PathMode::Surface,
            points: points.to_vec(),
            smoothing: 0.,
            closed: true,
            seam: 0,
            surface_signature: 0,
            attached_route: None,
            corner_rounding: Vec::new(),
        };
        assert!(path.sample(0.025).is_ok(), "{:?}", path.sample(0.025).err());
    }

    #[test]
    fn level_ring_tracks_touch_height_on_cylinder_taper_and_bent_body() {
        for (taper, bend) in [(0., 0.), (0.3, 0.), (0.2, 0.35)] {
            let mesh = cylinder(taper, bend);
            for height in [-0.5, 0.13, 0.5] {
                let hit = hit(&mesh, height);
                let points = section_ring(&mesh, hit, Vec3(0., 1., 0.)).unwrap();
                check_loop(&mesh, &points, hit);
                assert!(points.iter().all(|p| (p.position.1 - height).abs() < 1e-5));
                assert!(perimeter(&points) > 4.);
            }
        }
    }

    #[test]
    fn shortest_ring_stays_level_on_a_straight_cylinder_at_rows_and_between_rows() {
        let mesh = cylinder(0., 0.);
        for height in [-0.25, 0.13, 0.5] {
            let hit = hit(&mesh, height);
            let points = shortest_ring(&mesh, hit, Vec3(0., 1., 0.)).unwrap();
            check_loop(&mesh, &points, hit);
            assert!(points.iter().all(|p| (p.position.1 - height).abs() < 1e-4));
            let level = section_ring(&mesh, hit, Vec3(0., 1., 0.)).unwrap();
            assert!((perimeter(&points) - perimeter(&level)).abs() < 1e-3);
        }
    }

    #[test]
    fn shortest_ring_finds_a_shorter_nonlevel_route_on_an_uneven_body() {
        let mut mesh = cylinder(0., 0.);
        for p in &mut mesh.vertices {
            let radius = 0.6 + 0.8 * (1. - p.1.abs() * 2.).max(0.);
            p.0 *= radius;
            p.2 *= radius;
        }
        let hit = hit(&mesh, 0.);
        let level = section_ring(&mesh, hit, Vec3(0., 1., 0.)).unwrap();
        let shortest = shortest_ring(&mesh, hit, Vec3(0., 1., 0.)).unwrap();
        check_loop(&mesh, &shortest, hit);
        assert!(shortest.iter().any(|p| p.position.1.abs() > 0.2));
        assert!(perimeter(&shortest) < perimeter(&level) * 0.9);
        let winding: f32 = (0..shortest.len())
            .map(|i| {
                let a = Vec3(shortest[i].position.0, 0., shortest[i].position.2);
                let b = Vec3(
                    shortest[(i + 1) % shortest.len()].position.0,
                    0.,
                    shortest[(i + 1) % shortest.len()].position.2,
                );
                Vec3(0., 1., 0.).dot(a.cross(b)).atan2(a.dot(b))
            })
            .sum();
        assert!((winding - std::f32::consts::TAU).abs() < 1e-4);
    }

    fn append_shell(mesh: &mut Mesh, other: &Mesh, offset: Vec3, scale: f32) {
        let first = mesh.vertices.len();
        mesh.vertices
            .extend(other.vertices.iter().map(|&p| p * scale + offset));
        mesh.mask.extend(vec![0.; other.vertices.len()]);
        mesh.triangles
            .extend(other.triangles.iter().map(|face| face.map(|i| i + first)));
    }

    #[test]
    fn contour_selection_stays_on_touched_shell_and_rejects_coincident_shells() {
        let original = cylinder(0., 0.);
        let hit = hit(&original, 0.13);
        let mut nested = original.clone();
        append_shell(&mut nested, &original, Vec3::default(), 0.5);
        append_shell(&mut nested, &original, Vec3(4., 0., 0.), 1.);
        for points in [
            section_ring(&nested, hit, Vec3(0., 1., 0.)).unwrap(),
            shortest_ring(&nested, hit, Vec3(0., 1., 0.)).unwrap(),
        ] {
            assert!(points.iter().all(|p| p.position.0.abs() <= 1.001));
            assert!(perimeter(&points) > 6.);
        }
        let mut ambiguous = original.clone();
        append_shell(&mut ambiguous, &original, Vec3::default(), 1.);
        assert!(
            section_ring(&ambiguous, hit, Vec3(0., 1., 0.))
                .unwrap_err()
                .contains("ambiguous")
        );
    }

    #[test]
    fn open_hidden_cut_cap_and_cancel_are_clear_failures() {
        let mut mesh = cylinder(0., 0.);
        let touch = hit(&mesh, 0.13);
        mesh.hidden_faces = mesh
            .triangles
            .iter()
            .map(|face| face.iter().all(|&i| mesh.vertices[i].0 < -0.8))
            .collect();
        assert!(
            section_ring(&mesh, touch, Vec3(0., 1., 0.))
                .unwrap_err()
                .contains("open")
        );
        mesh.hidden_faces.clear();
        assert!(
            section_ring(&mesh, hit(&mesh, 1.), Vec3(0., 1., 0.))
                .unwrap_err()
                .contains("singular")
        );
        assert!(section_ring(&mesh, touch, Vec3::default()).is_err());
        assert!(
            shortest_ring_cancelled(&mesh, touch, Vec3(0., 1., 0.), &|| true)
                .unwrap_err()
                .contains("cancelled")
        );
    }
}
