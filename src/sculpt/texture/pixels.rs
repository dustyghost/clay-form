use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Precision {
    #[default]
    U16,
    Float32,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Pixels {
    U16(Arc<Vec<u16>>),
    Float32(Arc<Vec<f32>>),
}
impl Pixels {
    pub fn precision(&self) -> Precision {
        match self {
            Self::U16(_) => Precision::U16,
            Self::Float32(_) => Precision::Float32,
        }
    }
    pub fn len(&self) -> usize {
        match self {
            Self::U16(p) => p.len(),
            Self::Float32(p) => p.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn byte_len(&self) -> usize {
        self.len()
            * if self.precision() == Precision::U16 {
                2
            } else {
                4
            }
    }
    pub fn storage_id(&self) -> usize {
        match self {
            Self::U16(p) => Arc::as_ptr(p) as usize,
            Self::Float32(p) => Arc::as_ptr(p) as usize,
        }
    }
    pub fn pixel(&self, index: usize) -> [f32; 4] {
        match self {
            Self::U16(p) => std::array::from_fn(|c| p[index * 4 + c] as f32 / 65535.),
            Self::Float32(p) => p[index * 4..index * 4 + 4].try_into().unwrap(),
        }
    }
    pub fn set_pixel(&mut self, index: usize, value: [f32; 4]) -> bool {
        match self {
            Self::U16(p) => {
                let value = value.map(|v| (v.clamp(0., 1.) * 65535.).round() as u16);
                if p[index * 4..index * 4 + 4] == value {
                    return false;
                }
                Arc::make_mut(p)[index * 4..index * 4 + 4].copy_from_slice(&value);
            }
            Self::Float32(p) => {
                if p[index * 4..index * 4 + 4] == value {
                    return false;
                }
                Arc::make_mut(p)[index * 4..index * 4 + 4].copy_from_slice(&value);
            }
        }
        true
    }
    pub fn u16(&self) -> &[u16] {
        let Self::U16(p) = self else {
            panic!("expected integer pixels")
        };
        p
    }
    #[cfg(test)]
    pub fn u16_mut(&mut self) -> &mut Vec<u16> {
        let Self::U16(p) = self else {
            panic!("expected integer pixels")
        };
        Arc::make_mut(p)
    }
}
