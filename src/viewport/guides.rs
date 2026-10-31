//! Surface symmetry guides, independent of sculpt edits and gesture handling.
use super::scene::Scene;
use crate::sculpt::{Mesh, Symmetry, Vec3};
use eframe::egui::{self, Color32};
pub(super) fn paint(
    scene: &Scene,
    painter: &egui::Painter,
    mesh: &Mesh,
    symmetry: &Symmetry,
    opacity: f32,
) {
    for guide in symmetry.guides() {
        let normal = guide.normal;
        let color = if guide.side.is_some() {
            Color32::from_rgb(255, 215, 115)
        } else if normal.0.abs() > 0.99999 {
            Color32::from_rgb(255, 120, 140)
        } else if normal.1.abs() > 0.99999 {
            Color32::from_rgb(135, 245, 160)
        } else if normal.2.abs() > 0.99999 {
            Color32::from_rgb(130, 190, 255)
        } else {
            Color32::from_rgb(255, 215, 115)
        };
        for &(face, _) in scene.active_faces() {
            let triangle = mesh.triangles[face].map(|i| mesh.vertices[i]);
            for segment in plane_segments(triangle, normal, symmetry.center) {
                let Some([a, b]) = clip_guide(segment, guide.side, symmetry.center) else {
                    continue;
                };
                let Some(line) = scene.clip_segment([a, b]) else {
                    continue;
                };
                painter.line_segment(
                    line,
                    egui::Stroke::new(3_f32, Color32::from_black_alpha((opacity * 150.) as u8)),
                );
                painter.line_segment(
                    line,
                    egui::Stroke::new(1.5_f32, color.gamma_multiply(opacity)),
                );
            }
        }
    }
}
fn clip_guide(mut segment: [Vec3; 2], side: Option<Vec3>, center: Vec3) -> Option<[Vec3; 2]> {
    let Some(side) = side else {
        return Some(segment);
    };
    let a = (segment[0] - center).dot(side);
    let b = (segment[1] - center).dot(side);
    if a < -1e-6 && b < -1e-6 {
        return None;
    }
    if a < -1e-6 || b < -1e-6 {
        let intersection = segment[0] + (segment[1] - segment[0]) * (a / (a - b));
        if a < -1e-6 {
            segment[0] = intersection;
        } else {
            segment[1] = intersection;
        }
    }
    Some(segment)
}

/// Triangle/plane intersection, including vertices and edges exactly on the plane.
fn plane_segments(triangle: [Vec3; 3], normal: Vec3, center: Vec3) -> Vec<[Vec3; 2]> {
    const EPS: f32 = 1e-6;
    let distances = triangle.map(|v| (v - center).dot(normal));
    if distances.iter().all(|d| d.abs() < EPS) {
        return vec![
            [triangle[0], triangle[1]],
            [triangle[1], triangle[2]],
            [triangle[2], triangle[0]],
        ];
    }
    let mut points = Vec::new();
    for (i, j) in [(0, 1), (1, 2), (2, 0)] {
        let (a, b) = (distances[i], distances[j]);
        if a.abs() < EPS {
            points.push(triangle[i]);
        }
        if a * b < 0. {
            points.push(triangle[i] + (triangle[j] - triangle[i]) * (a / (a - b)));
        }
    }
    let mut unique: Vec<Vec3> = Vec::new();
    for point in points {
        if !unique.iter().any(|p| (*p - point).len() < EPS) {
            unique.push(point);
        }
    }
    if unique.len() >= 2 {
        vec![[unique[0], unique[1]]]
    } else {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn radial_boundaries_are_clipped_to_one_side_of_the_axis() {
        let segment = [Vec3(-1., 0., 0.), Vec3(1., 0., 0.)];
        let clipped = clip_guide(segment, Some(Vec3(1., 0., 0.)), Vec3::default()).unwrap();
        assert_eq!(clipped, [Vec3::default(), Vec3(1., 0., 0.)]);
        assert!(clip_guide(segment, Some(Vec3(1., 0., 0.)), Vec3(2., 0., 0.)).is_none());
    }
    #[test]
    fn guides_intersect_the_surface_and_handle_vertices_on_the_plane() {
        let normal = Vec3(1., 0., 0.);
        let segments = plane_segments(
            [Vec3(-1., 0., 1.), Vec3(1., 0., 1.), Vec3(1., 1., 1.)],
            normal,
            Vec3::default(),
        );
        assert_eq!(segments.len(), 1);
        for point in segments[0] {
            assert_eq!(point.0, 0.);
            assert_eq!(point.2, 1.);
        }
        let on_edge = plane_segments(
            [Vec3(0., 0., 1.), Vec3(0., 1., 1.), Vec3(1., 1., 1.)],
            normal,
            Vec3::default(),
        );
        assert_eq!(on_edge, vec![[Vec3(0., 0., 1.), Vec3(0., 1., 1.)]]);
        assert!(
            plane_segments(
                [Vec3(1., 0., 0.), Vec3(2., 0., 0.), Vec3(1., 1., 0.)],
                normal,
                Vec3::default()
            )
            .is_empty()
        );
    }
}
