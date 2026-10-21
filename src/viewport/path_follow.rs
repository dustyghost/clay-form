//! A live stroke follows only the portion of a saved route traversed by the
//! pointer. Arc parameters stay unwrapped across a closed route's seam.
use crate::sculpt::{Vec3, paths::PathSample};
use eframe::egui::Pos2;

const MAX_DABS_PER_UPDATE: usize = 4096;

pub(crate) struct PathRoute {
    samples: Vec<PathSample>,
    closed: bool,
    length: f32,
}

impl PathRoute {
    /// Closed routes use the uniformly spaced, nonduplicated samples returned
    /// by SurfacePath::samples_on_mesh. Open routes include both endpoints.
    pub(crate) fn new(samples: Vec<PathSample>, closed: bool) -> Result<Self, String> {
        let finite = |p: Vec3| p.0.is_finite() && p.1.is_finite() && p.2.is_finite();
        if samples.len() < if closed { 3 } else { 2 }
            || samples.iter().any(|p| {
                !finite(p.position)
                    || !finite(p.normal)
                    || !finite(p.tangent)
                    || !p.distance.is_finite()
                    || p.normal.len() < 0.5
                    || p.tangent.len() < 0.5
            })
            || samples[0].distance.abs() > 1e-6
            || samples.windows(2).any(|p| p[1].distance <= p[0].distance)
        {
            return Err("A brush guide needs a finite, ordered surface route".into());
        }
        let last = samples.last().unwrap().distance;
        let step = last / (samples.len() - 1) as f32;
        if closed
            && samples
                .windows(2)
                .any(|p| ((p[1].distance - p[0].distance) - step).abs() > step * 0.001)
        {
            return Err("A closed brush guide needs evenly spaced route samples".into());
        }
        let length = last + if closed { step } else { 0. };
        if !length.is_finite() || length < 1e-6 {
            return Err("The brush guide has no usable length".into());
        }
        Ok(Self {
            samples,
            closed,
            length,
        })
    }

    pub(crate) fn length(&self) -> f32 {
        self.length
    }

    fn bounded(&self, distance: f32) -> f32 {
        if self.closed {
            distance
        } else {
            distance.clamp(0., self.length)
        }
    }

    pub(crate) fn sample(&self, distance: f32) -> PathSample {
        let distance = self.bounded(distance);
        let local = if self.closed {
            distance.rem_euclid(self.length)
        } else {
            distance
        };
        let index = self
            .samples
            .partition_point(|p| p.distance <= local)
            .saturating_sub(1)
            .min(self.samples.len() - 1);
        let a = self.samples[index];
        let next = if index + 1 < self.samples.len() {
            index + 1
        } else if self.closed {
            0
        } else {
            index
        };
        let b = self.samples[next];
        let end = if index + 1 < self.samples.len() {
            b.distance
        } else {
            self.length
        };
        let t = ((local - a.distance) / (end - a.distance).max(f32::MIN_POSITIVE)).clamp(0., 1.);
        let normal = (a.normal * (1. - t) + b.normal * t).norm();
        let direction = a.tangent * (1. - t) + b.tangent * t;
        let tangent = (direction - normal * direction.dot(normal)).norm();
        PathSample {
            position: a.position * (1. - t) + b.position * t,
            normal,
            tangent,
            binormal: normal.cross(tangent).norm(),
            distance,
        }
    }

    /// The projection must return None for rear-facing or occluded points.
    /// During a stroke, max_travel bounds the local route neighborhood, and
    /// near-equal screen hits prefer continuity over a different crossing.
    pub(crate) fn nearest(
        &self,
        pointer: Pos2,
        project: impl Fn(&PathSample) -> Option<Pos2>,
        previous: Option<f32>,
        max_pixels: f32,
        max_travel: f32,
    ) -> Option<f32> {
        if !pointer.x.is_finite()
            || !pointer.y.is_finite()
            || !max_pixels.is_finite()
            || max_pixels < 0.
            || previous.is_some_and(|p| !p.is_finite())
            || (previous.is_some() && (!max_travel.is_finite() || max_travel <= 0.))
        {
            return None;
        }
        let mut best: Option<(f32, f32, f32)> = None;
        let segments = self.samples.len() - usize::from(!self.closed);
        for index in 0..segments {
            let a = self.samples[index];
            let b = self.samples[(index + 1) % self.samples.len()];
            let (Some(pa), Some(pb)) = (project(&a), project(&b)) else {
                continue;
            };
            let edge = pb - pa;
            if edge.length_sq() < 1e-8 {
                continue;
            }
            let t = ((pointer - pa).dot(edge) / edge.length_sq()).clamp(0., 1.);
            let pixels = (pa + edge * t).distance(pointer);
            if !pixels.is_finite() || pixels > max_pixels {
                continue;
            }
            let end = if index + 1 < self.samples.len() {
                b.distance
            } else {
                self.length
            };
            let mut parameter = a.distance + (end - a.distance) * t;
            if let Some(previous) = previous {
                if self.closed {
                    parameter += ((previous - parameter) / self.length).round() * self.length;
                }
                if (parameter - previous).abs() > max_travel {
                    continue;
                }
            }
            // Recheck the actual hit; visible endpoints can span an occluder.
            if project(&self.sample(parameter)).is_none() {
                continue;
            }
            let travel = previous.map_or(0., |p| (parameter - p).abs());
            let replace = best.is_none_or(|(best_pixels, best_travel, _)| {
                if previous.is_some() && (pixels - best_pixels).abs() <= 1.5 {
                    travel < best_travel
                } else {
                    pixels < best_pixels
                }
            });
            if replace {
                best = Some((pixels, travel, parameter));
            }
        }
        best.map(|(_, _, parameter)| parameter)
    }
}

pub(crate) struct PathCursor {
    distance: f32,
    remainder: f32,
    last_emitted: f32,
    direction: f32,
}

impl PathCursor {
    /// The caller emits the first dab once; subsequent updates do not repeat it.
    pub(crate) fn new(distance: f32) -> Self {
        Self {
            distance,
            remainder: 0.,
            last_emitted: distance,
            direction: 1.,
        }
    }
    pub(crate) fn distance(&self) -> f32 {
        self.distance
    }

    /// Spacing follows accumulated travel, including direction reversals. A
    /// whole-loop crossing has no special endpoint dab. Failure is atomic.
    pub(crate) fn advance(
        &mut self,
        route: &PathRoute,
        to: f32,
        spacing: f32,
    ) -> Result<Vec<PathSample>, String> {
        if !to.is_finite() || !self.distance.is_finite() || !spacing.is_finite() || spacing <= 0. {
            return Err("Live path spacing and movement must be finite and positive".into());
        }
        let from = route.bounded(self.distance);
        let to = route.bounded(to);
        let delta = to - from;
        let travel = delta.abs();
        // If radius/spacing changes while drawing, retain only the unconsumed
        // fraction at the new spacing rather than producing stationary dabs.
        let remainder = self.remainder.rem_euclid(spacing);
        let total = remainder + travel;
        let count = (total / spacing + 1e-5).floor() as usize;
        if count > MAX_DABS_PER_UPDATE {
            return Err("Live path movement exceeds the dab limit; increase spacing".into());
        }
        let mut result = Vec::with_capacity(count);
        for index in 0..count {
            let offset = (spacing - remainder + spacing * index as f32).min(travel);
            let mut sample = route.sample(from + delta.signum() * offset);
            if delta < 0. {
                sample.tangent = sample.tangent * -1.;
                sample.binormal = sample.binormal * -1.;
            }
            result.push(sample);
        }
        self.distance = to;
        self.remainder = (total - spacing * count as f32).max(0.);
        if travel > 0. {
            self.direction = delta.signum();
        }
        if let Some(sample) = result.last() {
            self.last_emitted = sample.distance;
        }
        Ok(result)
    }

    /// Optional release endpoint for brushes whose delta must reach the exact
    /// pointer location. Ordinary stamps may simply retain their normal spacing.
    pub(crate) fn flush_end(&mut self, route: &PathRoute) -> Option<PathSample> {
        let delta = (self.distance - self.last_emitted).abs();
        let delta = if route.closed {
            let local = delta.rem_euclid(route.length);
            local.min(route.length - local)
        } else {
            delta
        };
        self.remainder = 0.;
        if delta < 1e-6 {
            return None;
        }
        let mut sample = route.sample(self.distance);
        if self.direction < 0. {
            sample.tangent = sample.tangent * -1.;
            sample.binormal = sample.binormal * -1.;
        }
        self.last_emitted = self.distance;
        Some(sample)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(position: Vec3, tangent: Vec3, distance: f32) -> PathSample {
        let normal = Vec3(0., 0., 1.);
        PathSample {
            position,
            normal,
            tangent,
            binormal: normal.cross(tangent),
            distance,
        }
    }
    fn line() -> PathRoute {
        PathRoute::new(
            (0..=100)
                .map(|i| {
                    sample(
                        Vec3(i as f32 * 0.1, 0., 0.),
                        Vec3(1., 0., 0.),
                        i as f32 * 0.1,
                    )
                })
                .collect(),
            false,
        )
        .unwrap()
    }
    fn circle() -> PathRoute {
        PathRoute::new(
            (0..128)
                .map(|i| {
                    let angle = std::f32::consts::TAU * i as f32 / 128.;
                    sample(
                        Vec3(angle.cos(), angle.sin(), 0.),
                        Vec3(-angle.sin(), angle.cos(), 0.),
                        angle,
                    )
                })
                .collect(),
            true,
        )
        .unwrap()
    }
    fn project(sample: &PathSample) -> Option<Pos2> {
        Some(Pos2::new(
            sample.position.0 * 100.,
            sample.position.1 * 100.,
        ))
    }

    #[test]
    fn route_interpolates_arc_distance_and_clamps_open_endpoints() {
        let route = line();
        assert_eq!(route.length(), 10.);
        for distance in [-1., 0.37, 9.94, 11.] {
            let p = route.sample(distance);
            assert!((p.position.0 - distance.clamp(0., 10.)).abs() < 1e-5);
            assert!(p.tangent.cross(p.binormal).dot(p.normal) > 0.999);
        }
    }

    #[test]
    fn sparse_and_frequent_pointer_events_emit_the_same_spaced_dabs() {
        let route = line();
        let mut sparse = PathCursor::new(0.);
        let a = sparse.advance(&route, 1., 0.1).unwrap();
        let mut frequent = PathCursor::new(0.);
        let mut b = Vec::new();
        for i in 1..=25 {
            b.extend(frequent.advance(&route, i as f32 * 0.04, 0.1).unwrap());
        }
        assert_eq!(a.len(), 10);
        assert_eq!(a.len(), b.len());
        for (a, b) in a.iter().zip(&b) {
            assert!((a.distance - b.distance).abs() < 1e-5);
        }
        assert!(a[0].distance > 0.);
        assert!(frequent.flush_end(&route).is_none());
    }

    #[test]
    fn reversing_preserves_travel_remainder_and_exact_optional_endpoint_delta() {
        let route = line();
        let mut cursor = PathCursor::new(0.);
        assert!(cursor.advance(&route, 0.06, 0.1).unwrap().is_empty());
        let reversed = cursor.advance(&route, 0., 0.1).unwrap();
        assert_eq!(reversed.len(), 1);
        assert!((reversed[0].distance - 0.02).abs() < 1e-6);
        assert!(reversed[0].tangent.0 < -0.99);
        let end = cursor.flush_end(&route).unwrap();
        assert_eq!(end.distance, 0.);
        assert!((end.position - reversed[0].position).0 < 0.);
        assert!(cursor.flush_end(&route).is_none());
    }

    #[test]
    fn seam_tracking_unwraps_and_dabs_cross_once_without_endpoint_duplication() {
        let route = circle();
        let prior = route.length() - 0.03;
        let target = route.sample(0.02);
        let next = route
            .nearest(project(&target).unwrap(), project, Some(prior), 12., 0.2)
            .unwrap();
        assert!((next - (route.length() + 0.02)).abs() < 1e-4);
        let mut cursor = PathCursor::new(route.length() - 0.1);
        let dabs = cursor.advance(&route, route.length() + 0.1, 0.05).unwrap();
        assert_eq!(dabs.len(), 4);
        assert_eq!(
            dabs.iter()
                .filter(|p| (p.distance - route.length()).abs() < 1e-5)
                .count(),
            1
        );
        assert!(cursor.flush_end(&route).is_none());
        let backward = route
            .nearest(
                project(&route.sample(-0.02)).unwrap(),
                project,
                Some(0.03),
                12.,
                0.2,
            )
            .unwrap();
        assert!((backward + 0.02).abs() < 1e-4);
    }

    #[test]
    fn crossing_selection_stays_on_the_local_branch() {
        let positions = [
            Vec3(-1., -1., 0.),
            Vec3(1., 1., 0.),
            Vec3(-1., 1., 0.),
            Vec3(1., -1., 0.),
        ];
        let mut distance = 0.;
        let samples = positions
            .iter()
            .enumerate()
            .map(|(i, &p)| {
                if i > 0 {
                    distance += (p - positions[i - 1]).len();
                }
                let tangent = if i + 1 < positions.len() {
                    positions[i + 1] - p
                } else {
                    p - positions[i - 1]
                };
                sample(p, tangent.norm(), distance)
            })
            .collect();
        let route = PathRoute::new(samples, false).unwrap();
        let first_crossing = 2_f32.sqrt();
        let hit = route
            .nearest(Pos2::ZERO, project, Some(first_crossing - 0.05), 10., 0.2)
            .unwrap();
        assert!((hit - first_crossing).abs() < 1e-5);
        assert!(
            route
                .nearest(
                    Pos2::new(100., -100.),
                    project,
                    Some(first_crossing),
                    10.,
                    0.2
                )
                .is_none()
        );
    }

    #[test]
    fn rear_surface_cannot_capture_or_steal_the_front_route() {
        let positions = [
            Vec3(-1., 0., 1.),
            Vec3(1., 0., 1.),
            Vec3(1., 0., -1.),
            Vec3(-1., 0., -1.),
        ];
        let samples = positions
            .iter()
            .enumerate()
            .map(|(i, &position)| {
                let mut p = sample(position, Vec3(1., 0., 0.), i as f32 * 2.);
                p.normal = Vec3(0., 0., position.2);
                p
            })
            .collect();
        let route = PathRoute::new(samples, false).unwrap();
        let visible = |p: &PathSample| if p.normal.2 > 0. { project(p) } else { None };
        assert!((route.nearest(Pos2::ZERO, visible, None, 10., 1.).unwrap() - 1.).abs() < 1e-6);
        assert!(
            route
                .nearest(Pos2::ZERO, visible, Some(5.), 10., 0.5)
                .is_none()
        );
    }

    #[test]
    fn live_movement_limits_are_atomic_and_stationary_spacing_change_adds_no_dab() {
        let route = line();
        let mut cursor = PathCursor::new(0.);
        assert!(cursor.advance(&route, 10., 1e-8).is_err());
        assert_eq!(cursor.distance(), 0.);
        assert!(cursor.advance(&route, 0.09, 0.1).unwrap().is_empty());
        assert!(cursor.advance(&route, 0.09, 0.04).unwrap().is_empty());
        assert!(cursor.advance(&route, f32::NAN, 0.04).is_err());
        assert_eq!(cursor.distance(), 0.09);
    }
}
