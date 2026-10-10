use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    Standard,
    Clay,
    Inflate,
    Smooth,
    Flatten,
    Pinch,
    Crease,
    Grab,
    Mask,
}
impl Kind {
    pub const ALL: [Self; 9] = [
        Self::Standard,
        Self::Clay,
        Self::Inflate,
        Self::Smooth,
        Self::Flatten,
        Self::Pinch,
        Self::Crease,
        Self::Grab,
        Self::Mask,
    ];
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Alpha {
    Soft,
    Hard,
    Square,
    Image {
        width: usize,
        height: usize,
        pixels: Arc<[f32]>,
    },
}
impl PartialEq for Alpha {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Soft, Self::Soft) | (Self::Hard, Self::Hard) | (Self::Square, Self::Square) => {
                true
            }
            (
                Self::Image {
                    width: a,
                    height: b,
                    pixels: c,
                },
                Self::Image {
                    width: x,
                    height: y,
                    pixels: z,
                },
            ) => a == x && b == y && (Arc::ptr_eq(c, z) || c.as_ref() == z.as_ref()),
            _ => false,
        }
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Brush {
    pub name: String,
    pub kind: Kind,
    pub alpha: Alpha,
    pub radius: f32,
    pub strength: f32,
    pub falloff: f32,
    pub spacing: f32,
    pub rotation: f32,
    pub subtract: bool,
    pub pressure: bool,
    pub pressure_radius: bool,
    pub pressure_curve: f32,
}
impl Brush {
    pub fn new(kind: Kind) -> Self {
        Self {
            name: format!("{kind:?}"),
            kind,
            alpha: Alpha::Soft,
            radius: 60.,
            strength: 0.25,
            falloff: 1.5,
            spacing: 0.15,
            rotation: 0.,
            subtract: false,
            pressure: true,
            pressure_radius: false,
            pressure_curve: 1.,
        }
    }
    pub fn weight(&self, x: f32, y: f32) -> f32 {
        let (s, c) = self.rotation.to_radians().sin_cos();
        let (x, y) = (x * c - y * s, x * s + y * c);
        let r = (x * x + y * y).sqrt();
        if r >= 1. {
            return 0.;
        }
        match &self.alpha {
            Alpha::Soft => (1. - r).powf(self.falloff),
            Alpha::Hard => 1.,
            Alpha::Square => {
                if x.abs().max(y.abs()) < 0.7 {
                    1.
                } else {
                    0.
                }
            }
            Alpha::Image {
                width,
                height,
                pixels,
            } => {
                let ix = (((x + 1.) * 0.5 * (*width - 1) as f32) as usize).min(*width - 1);
                let iy = (((1. - y) * 0.5 * (*height - 1) as f32) as usize).min(*height - 1);
                pixels[iy * width + ix] * (1. - r).powf(self.falloff)
            }
        }
    }
}
pub fn validate_brush(b: &Brush) -> Result<(), String> {
    if b.name.len() > 256
        || !b.radius.is_finite()
        || !(1. ..=500.).contains(&b.radius)
        || !b.strength.is_finite()
        || !(0. ..=1.).contains(&b.strength)
        || !b.falloff.is_finite()
        || !(0.1..=8.).contains(&b.falloff)
        || !b.spacing.is_finite()
        || !(0.01..=1.).contains(&b.spacing)
        || !b.rotation.is_finite()
        || !b.pressure_curve.is_finite()
        || !(0.1..=4.).contains(&b.pressure_curve)
    {
        return Err("Invalid brush settings".into());
    }
    if let Alpha::Image {
        width,
        height,
        pixels,
    } = &b.alpha
        && (*width == 0
            || *height == 0
            || *width > 2048
            || *height > 2048
            || pixels.len() != width * height
            || pixels
                .iter()
                .any(|x| !x.is_finite() || !(0. ..=1.).contains(x)))
    {
        return Err("Invalid alpha".into());
    }
    Ok(())
}
