//! Cross-section size sampled by distance along an assembly's path.
use serde::{Deserialize, Serialize};

// Enough for every path point and repeat, plus user-placed profile points.
pub const MAX_THICKNESS_POINTS: usize = 4096;
pub const MIN_THICKNESS: f32 = 0.01;
pub const MAX_THICKNESS: f32 = 10.;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThicknessMode {
    #[default]
    Uniform,
    Taper,
    Profile,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThicknessPoint {
    pub position: f32,
    pub factor: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Thickness {
    pub mode: ThicknessMode,
    pub uniform: f32,
    pub start: f32,
    pub end: f32,
    pub smooth: bool,
    pub points: Vec<ThicknessPoint>,
}
impl Default for Thickness {
    fn default() -> Self {
        Self {
            mode: ThicknessMode::Uniform,
            uniform: 1.,
            start: 1.,
            end: 1.,
            smooth: true,
            points: [0., 0.5, 1.]
                .map(|position| ThicknessPoint {
                    position,
                    factor: 1.,
                })
                .to_vec(),
        }
    }
}
impl Thickness {
    pub fn validate(&self) -> Result<(), String> {
        let valid = |v: f32| v.is_finite() && (MIN_THICKNESS..=MAX_THICKNESS).contains(&v);
        if ![self.uniform, self.start, self.end].into_iter().all(valid)
            || !(2..=MAX_THICKNESS_POINTS).contains(&self.points.len())
            || self.points.iter().any(|p| {
                !p.position.is_finite() || !(0. ..=1.).contains(&p.position) || !valid(p.factor)
            })
            || self.points.first().is_none_or(|p| p.position != 0.)
            || self.points.last().is_none_or(|p| p.position != 1.)
            || self
                .points
                .windows(2)
                .any(|p| p[1].position <= p[0].position)
        {
            return Err("Thickness needs 2–4096 ordered points from 0% to 100%, with sizes between 0.01× and 10×".into());
        }
        Ok(())
    }

    pub fn edit_at(&mut self, position: f32, factor: f32, closed: bool) -> Result<(), String> {
        self.validate()?;
        if !position.is_finite()
            || !factor.is_finite()
            || !(MIN_THICKNESS..=MAX_THICKNESS).contains(&factor)
        {
            return Err("Invalid thickness handle value".into());
        }
        let t = if closed {
            position.rem_euclid(1.)
        } else {
            position.clamp(0., 1.)
        };
        let endpoint = t < 1e-5 || (!closed && t > 1. - 1e-5);
        if endpoint && self.mode == ThicknessMode::Uniform {
            self.start = self.uniform;
            self.end = self.uniform;
            self.mode = ThicknessMode::Taper;
        }
        if endpoint && self.mode == ThicknessMode::Taper {
            if t < 0.5 {
                self.start = factor;
            } else {
                self.end = factor;
            }
            return Ok(());
        }
        if self.mode != ThicknessMode::Profile {
            let mut points = vec![ThicknessPoint {
                position: 0.,
                factor: self.at(0., closed),
            }];
            if closed && self.mode == ThicknessMode::Taper {
                points.push(ThicknessPoint {
                    position: 0.5,
                    factor: self.end,
                });
            }
            points.push(ThicknessPoint {
                position: 1.,
                factor: self.at(1., closed),
            });
            self.points = points;
            self.mode = ThicknessMode::Profile;
        }
        if let Some(point) = self
            .points
            .iter_mut()
            .find(|p| (p.position - t).abs() < 1e-5)
        {
            point.factor = factor;
        } else {
            if self.points.len() >= MAX_THICKNESS_POINTS {
                return Err(
                    "Thickness profile point limit reached; remove a point before adding another"
                        .into(),
                );
            }
            let index = self.points.partition_point(|p| p.position < t);
            self.points.insert(
                index,
                ThicknessPoint {
                    position: t,
                    factor,
                },
            );
        }
        self.validate()
    }

    /// Called after validation. Closed profiles wrap at the seam; their final
    /// endpoint uses the first value, leaving open-path settings untouched.
    pub fn at(&self, position: f32, closed: bool) -> f32 {
        let t = if closed {
            position.rem_euclid(1.)
        } else {
            position.clamp(0., 1.)
        };
        let mix = |a: f32, b: f32, t: f32| {
            let t = if self.smooth {
                t * t * (3. - 2. * t)
            } else {
                t
            };
            a * (1. - t) + b * t
        };
        match self.mode {
            ThicknessMode::Uniform => self.uniform,
            ThicknessMode::Taper => {
                let t = if closed { 1. - (2. * t - 1.).abs() } else { t };
                mix(self.start, self.end, t)
            }
            ThicknessMode::Profile => {
                let i = self
                    .points
                    .partition_point(|p| p.position < t)
                    .clamp(1, self.points.len() - 1);
                let (a, b) = (self.points[i - 1], self.points[i]);
                let end = if closed && i + 1 == self.points.len() {
                    self.points[0].factor
                } else {
                    b.factor
                };
                mix(a.factor, end, (t - a.position) / (b.position - a.position))
            }
        }
    }
}
