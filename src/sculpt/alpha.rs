//! Reusable image alphas and placed projection masks. No UI or filesystem dependencies.
use super::{Alpha, Dab, Symmetry, Vec3};
use serde::{Deserialize, Serialize};
mod builtins;
pub use builtins::builtins;

impl Alpha {
    pub fn validate(&self) -> Result<(), String> {
        if let Self::Image {
            width,
            height,
            pixels,
        } = self
            && (*width == 0
                || *height == 0
                || *width > 2048
                || *height > 2048
                || pixels.len() != width * height
                || pixels
                    .iter()
                    .any(|v| !v.is_finite() || !(0. ..=1.).contains(v)))
        {
            return Err(
                "Invalid alpha: use an image up to 2048 × 2048 with finite values between 0 and 1"
                    .into(),
            );
        }
        Ok(())
    }
    /// Rectangular image sampling, with zero coverage outside its bounds.
    pub fn sample(&self, u: f32, v: f32) -> f32 {
        if !(0. ..=1.).contains(&u) || !(0. ..=1.).contains(&v) {
            return 0.;
        }
        match self {
            Self::Image {
                width,
                height,
                pixels,
            } => {
                let x = u * (*width - 1) as f32;
                let y = v * (*height - 1) as f32;
                let (ix, iy) = (x.floor() as usize, y.floor() as usize);
                let (nx, ny) = ((ix + 1).min(*width - 1), (iy + 1).min(*height - 1));
                let (tx, ty) = (x - ix as f32, y - iy as f32);
                (pixels[iy * width + ix] * (1. - tx) + pixels[iy * width + nx] * tx) * (1. - ty)
                    + (pixels[ny * width + ix] * (1. - tx) + pixels[ny * width + nx] * tx) * ty
            }
            Self::Hard => f32::from((u * 2. - 1.).powi(2) + (v * 2. - 1.).powi(2) < 1.),
            Self::Square => f32::from((u * 2. - 1.).abs().max((v * 2. - 1.).abs()) < 0.7),
            Self::Soft => (1. - ((u * 2. - 1.).powi(2) + (v * 2. - 1.).powi(2)).sqrt()).max(0.),
        }
    }
    pub fn pixel_count(&self) -> usize {
        if let Self::Image { pixels, .. } = self {
            pixels.len()
        } else {
            0
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub category: String,
    pub image: Alpha,
}
impl Asset {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() || self.name.len() > 128 || self.category.len() > 128 {
            return Err("Alpha names must contain 1–128 bytes; categories at most 128".into());
        }
        self.image.validate()
    }
}
pub fn validate_library(assets: &[Asset]) -> Result<(), String> {
    if assets.len() > 64
        || assets.iter().map(|a| a.image.pixel_count()).sum::<usize>() > 4 * 2048 * 2048
    {
        return Err("Alpha library limit: 64 assets and 16 million image pixels".into());
    }
    for asset in assets {
        asset.validate()?;
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Adjustments {
    /// Left, top, right, bottom in source image coordinates, normalized to 0..1.
    pub crop: [f32; 4],
    pub black: f32,
    pub white: f32,
    pub gamma: f32,
    pub invert: bool,
    pub edge_fade: f32,
}
impl Default for Adjustments {
    fn default() -> Self {
        Self {
            crop: [0., 0., 1., 1.],
            black: 0.,
            white: 1.,
            gamma: 1.,
            invert: false,
            edge_fade: 0.,
        }
    }
}
impl Adjustments {
    pub fn apply(&self, source: &Alpha) -> Result<Alpha, String> {
        source.validate()?;
        if self
            .crop
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
            || self.crop[2] <= self.crop[0]
            || self.crop[3] <= self.crop[1]
            || !self.black.is_finite()
            || !self.white.is_finite()
            || self.black < 0.
            || self.white > 1.
            || self.white - self.black < 0.001
            || !self.gamma.is_finite()
            || !(0.1..=4.).contains(&self.gamma)
            || !self.edge_fade.is_finite()
            || !(0. ..=0.5).contains(&self.edge_fade)
        {
            return Err(
                "Crop must have positive width/height and white level must exceed black level"
                    .into(),
            );
        }
        let (w, h) = match source {
            Alpha::Image { width, height, .. } => (*width, *height),
            _ => (256, 256),
        };
        let width = ((self.crop[2] - self.crop[0]) * w as f32).round().max(1.) as usize;
        let height = ((self.crop[3] - self.crop[1]) * h as f32).round().max(1.) as usize;
        let mut pixels = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let u = if width == 1 {
                    0.5
                } else {
                    x as f32 / (width - 1) as f32
                };
                let v = if height == 1 {
                    0.5
                } else {
                    y as f32 / (height - 1) as f32
                };
                let mut value = ((source.sample(
                    self.crop[0] + u * (self.crop[2] - self.crop[0]),
                    self.crop[1] + v * (self.crop[3] - self.crop[1]),
                ) - self.black)
                    / (self.white - self.black))
                    .clamp(0., 1.)
                    .powf(self.gamma);
                if self.invert {
                    value = 1. - value;
                }
                if self.edge_fade > 0. {
                    let t = (u.min(1. - u).min(v).min(1. - v) / self.edge_fade).clamp(0., 1.);
                    value *= t * t * (3. - 2. * t);
                }
                pixels.push(value);
            }
        }
        Ok(Alpha::Image {
            width,
            height,
            pixels: pixels.into(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub origin: Vec3,
    pub right: Vec3,
    pub up: Vec3,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stencil {
    pub asset: Asset,
    pub enabled: bool,
    /// None follows the view. Some fixes a planar projector in model coordinates.
    pub surface: Option<Frame>,
    /// View: fraction of viewport's shorter side. Surface: model units.
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub rotation: f32,
    pub opacity: f32,
}
impl Stencil {
    pub fn new(asset: Asset) -> Self {
        Self {
            asset,
            enabled: true,
            surface: None,
            position: [0.; 2],
            size: [0.5; 2],
            rotation: 0.,
            opacity: 0.3,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        self.asset.validate()?;
        if self
            .position
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 10000.)
            || self
                .size
                .iter()
                .any(|v| !v.is_finite() || !(0.001..=10000.).contains(v))
            || !self.rotation.is_finite()
            || self.rotation.abs() > 36000.
            || !self.opacity.is_finite()
            || !(0. ..=1.).contains(&self.opacity)
        {
            return Err("Invalid stencil transform".into());
        }
        if let Some(f) = &self.surface
            && ([f.origin.0, f.origin.1, f.origin.2]
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1e6)
                || !f.right.len().is_finite()
                || !f.up.len().is_finite()
                || (f.right.len() - 1.).abs() > 0.001
                || (f.up.len() - 1.).abs() > 0.001
                || f.right.dot(f.up).abs() > 0.001)
        {
            return Err("Invalid surface stencil frame".into());
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq)]
pub struct Projection {
    pub image: Alpha,
    pub frame: Frame,
    pub view: Vec3,
    pub distance: f32,
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub rotation: f32,
}
impl Projection {
    pub fn weight(&self, point: Vec3) -> f32 {
        self.coordinates(point)
            .map_or(0., |[u, v]| self.image.sample(u, v))
    }
    fn coordinates(&self, point: Vec3) -> Option<[f32; 2]> {
        let p = point - self.frame.origin;
        let divisor = if self.distance == 0. {
            1.
        } else {
            1. - p.dot(self.view) / self.distance
        };
        if divisor <= 0.001 {
            return None;
        }
        let x = p.dot(self.frame.right) / divisor - self.position[0];
        let y = p.dot(self.frame.up) / divisor - self.position[1];
        let (s, c) = self.rotation.to_radians().sin_cos();
        Some([
            (x * c + y * s) / self.size[0] + 0.5,
            0.5 - (-x * s + y * c) / self.size[1],
        ])
    }
    /// Conservative bounds over the projected polygon. Looking inside its image
    /// bounds finds narrow bright features even when all existing vertices are black.
    /// A white lower bound permits simplification without crossing a protected gap.
    pub(crate) fn coverage_bounds(&self, points: &[Vec3]) -> (f32, f32) {
        let mut low = [f32::INFINITY; 2];
        let mut high = [f32::NEG_INFINITY; 2];
        let mut visible = 0;
        for &point in points {
            if let Some(uv) = self.coordinates(point) {
                visible += 1;
                for axis in 0..2 {
                    low[axis] = low[axis].min(uv[axis]);
                    high[axis] = high[axis].max(uv[axis]);
                }
            }
        }
        if visible == 0 {
            return (0., 0.);
        }
        if visible != points.len() {
            return (0., 1.);
        }
        if (0..2).any(|i| high[i] < 0. || low[i] > 1.) {
            return (0., 0.);
        }
        let outside = (0..2).any(|i| low[i] < 0. || high[i] > 1.);
        let low = low.map(|v| v.clamp(0., 1.));
        let high = high.map(|v| v.clamp(0., 1.));
        let (minimum, maximum) = match &self.image {
            Alpha::Image {
                width,
                height,
                pixels,
            } => {
                let x0 = (low[0] * (*width - 1) as f32).floor() as usize;
                let x1 = (high[0] * (*width - 1) as f32).ceil() as usize;
                let y0 = (low[1] * (*height - 1) as f32).floor() as usize;
                let y1 = (high[1] * (*height - 1) as f32).ceil() as usize;
                let mut minimum = 1_f32;
                let mut maximum = 0_f32;
                for y in y0..=y1 {
                    for &pixel in &pixels[y * width + x0..=y * width + x1] {
                        minimum = minimum.min(pixel);
                        maximum = maximum.max(pixel);
                    }
                    if minimum == 0. && maximum == 1. {
                        break;
                    }
                }
                (minimum, maximum)
            }
            Alpha::Square => (
                f32::from((0..2).all(|i| low[i] > 0.15 && high[i] < 0.85)),
                f32::from((0..2).all(|i| high[i] > 0.15 && low[i] < 0.85)),
            ),
            Alpha::Soft | Alpha::Hard => {
                let nearest = (0..2)
                    .map(|i| (0.5_f32.clamp(low[i], high[i]) * 2. - 1.).powi(2))
                    .sum::<f32>()
                    .sqrt();
                let farthest = (0..2)
                    .map(|i| {
                        (low[i] * 2. - 1.)
                            .abs()
                            .max((high[i] * 2. - 1.).abs())
                            .powi(2)
                    })
                    .sum::<f32>()
                    .sqrt();
                if self.image == Alpha::Hard {
                    (f32::from(farthest < 1.), f32::from(nearest < 1.))
                } else {
                    ((1. - farthest).max(0.), (1. - nearest).max(0.))
                }
            }
        };
        (if outside { 0. } else { minimum }, maximum)
    }
    pub fn copies(&self, symmetry: &Symmetry) -> Vec<Self> {
        symmetry
            .dabs(Dab {
                center: self.frame.origin,
                right: self.frame.right,
                up: self.frame.up,
                normal: self.view,
                radius: 1.,
                pressure: 1.,
                delta: Vec3::default(),
                invert: false,
            })
            .into_iter()
            .map(|d| Self {
                frame: Frame {
                    origin: d.center,
                    right: d.right,
                    up: d.up,
                },
                view: d.normal,
                ..self.clone()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
