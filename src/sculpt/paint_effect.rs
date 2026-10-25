//! Paint deposition and deterministic droplets in brush-local coordinates.
use super::Brush;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaintMode {
    #[default]
    Stroke,
    Airbrush,
    Spray,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PaintEffect {
    pub mode: PaintMode,
    pub flow: f32,
    pub density: f32,
    /// Dot radius as a fraction of the main brush radius.
    pub dot_size: f32,
    #[serde(skip)]
    pub(crate) seed: u32,
}
impl Default for PaintEffect {
    fn default() -> Self {
        Self {
            mode: PaintMode::Stroke,
            flow: 0.5,
            density: 0.4,
            dot_size: 0.035,
            seed: 0,
        }
    }
}
impl PaintEffect {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub fn valid(&self) -> bool {
        self.flow.is_finite()
            && (0.01..=1.).contains(&self.flow)
            && self.density.is_finite()
            && (0.02..=1.).contains(&self.density)
            && self.dot_size.is_finite()
            && (0.005..=0.2).contains(&self.dot_size)
    }
    fn droplets(&self, x: f32, y: f32) -> f32 {
        let size = self.dot_size * 4.;
        let x = x / size;
        let y = y / size;
        let cell = (x.floor() as i32 as u32).wrapping_mul(0x9e3779b9)
            ^ (y.floor() as i32 as u32).wrapping_mul(0x85ebca6b)
            ^ self.seed.wrapping_mul(0xc2b2ae35);
        let random = |salt: u32| {
            let mut h = cell ^ salt;
            h ^= h >> 16;
            h = h.wrapping_mul(0x7feb352d);
            h ^= h >> 15;
            h = h.wrapping_mul(0x846ca68b);
            h ^= h >> 16;
            (h >> 8) as f32 / 16777216.
        };
        if random(1) > self.density {
            return 0.;
        }
        let dx = (x - x.floor()) - (0.3 + random(2) * 0.4);
        let dy = (y - y.floor()) - (0.3 + random(3) * 0.4);
        (1. - (dx * dx + dy * dy) * 16.).clamp(0., 1.)
    }
}
impl Brush {
    pub(crate) fn paint_weight(&self, x: f32, y: f32) -> f32 {
        self.weight(x, y)
            * if self.paint_effect.mode == PaintMode::Spray {
                self.paint_effect.droplets(x, y)
            } else {
                1.
            }
    }
    pub(crate) fn continuous_paint(&self) -> bool {
        self.kind == super::Kind::Paint
            && self.paint_effect.mode != PaintMode::Stroke
            && self.effective_stroke_mode() == super::StrokeMode::Freehand
    }
    /// Exponential deposition gives the same airbrush coverage at different frame rates.
    pub(crate) fn paint_strength_for_time(&self, seconds: f32) -> f32 {
        // Sparse droplets need stronger individual deposits than a continuous disk.
        let rate = if self.paint_effect.mode == PaintMode::Spray {
            32.
        } else {
            8.
        };
        1. - (-self.strength * self.paint_effect.flow * seconds * rate).exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn airbrush_flow_is_time_based_and_spray_has_adjustable_separate_droplets() {
        let mut brush = Brush::new(super::super::Kind::Paint);
        brush.paint_effect.mode = PaintMode::Airbrush;
        let half = brush.paint_strength_for_time(0.5);
        let whole = brush.paint_strength_for_time(1.);
        assert!((whole - (1. - (1. - half).powi(2))).abs() < 1e-6);
        brush.paint_effect.flow = 1.;
        assert!(brush.paint_strength_for_time(1.) > whole);
        brush.paint_effect.mode = PaintMode::Spray;
        let samples = |b: &Brush| {
            (0..160)
                .flat_map(|y| {
                    (0..160)
                        .map(move |x| b.paint_weight(x as f32 / 100. - 0.8, y as f32 / 100. - 0.8))
                })
                .collect::<Vec<_>>()
        };
        let sparse = samples(&brush);
        assert!(sparse.iter().any(|v| *v > 0.));
        assert!(sparse.iter().filter(|v| **v == 0.).count() > sparse.len() / 2);
        brush.paint_effect.density = 1.;
        let dense = samples(&brush);
        assert!(
            dense.iter().filter(|v| **v > 0.).count() > sparse.iter().filter(|v| **v > 0.).count()
        );
        brush.paint_effect.seed += 1;
        assert_ne!(samples(&brush), dense);
        brush.paint_effect.dot_size = 0.1;
        assert_ne!(samples(&brush), dense);
        assert_eq!(brush.paint_weight(1.1, 0.), 0.);
    }
    #[test]
    fn paint_settings_roundtrip_and_old_presets_default_to_strokes() {
        let mut brush = Brush::new(super::super::Kind::Paint);
        let old = serde_json::to_value(&brush).unwrap();
        assert!(old.get("paint_effect").is_none());
        assert_eq!(
            serde_json::from_value::<Brush>(old)
                .unwrap()
                .paint_effect
                .mode,
            PaintMode::Stroke
        );
        brush.paint_effect.mode = PaintMode::Spray;
        brush.paint_effect.dot_size = 0.08;
        let restored: Brush = serde_json::from_slice(&serde_json::to_vec(&brush).unwrap()).unwrap();
        assert!(restored == brush);
        brush.paint_effect.flow = f32::NAN;
        assert!(super::super::validate_brush(&brush).is_err());
    }
}
