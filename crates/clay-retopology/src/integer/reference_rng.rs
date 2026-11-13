//! Local MT19937 plus the pinned Linux libstdc++ shuffle's draw schedule.
//! Fixed seed and explicit arithmetic make parity independent of host C++/libc.
pub(super) struct Rng {
    state: [u32; 624],
    index: usize,
}
impl Rng {
    pub fn new() -> Self {
        let mut state = [0u32; 624];
        for i in 1..624 {
            state[i] = 1812433253u32
                .wrapping_mul(state[i - 1] ^ (state[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        Self { state, index: 624 }
    }
    fn next(&mut self) -> u32 {
        if self.index == 624 {
            for i in 0..624 {
                let x = (self.state[i] & 0x80000000) | (self.state[(i + 1) % 624] & 0x7fffffff);
                self.state[i] = self.state[(i + 397) % 624]
                    ^ (x >> 1)
                    ^ if x & 1 != 0 { 0x9908b0df } else { 0 };
            }
            self.index = 0;
        }
        let mut x = self.state[self.index];
        self.index += 1;
        x ^= x >> 11;
        x ^= (x << 7) & 0x9d2c5680;
        x ^= (x << 15) & 0xefc60000;
        x ^= x >> 18;
        x
    }
    fn below(&mut self, range: u32) -> usize {
        loop {
            let product = u64::from(self.next()) * u64::from(range);
            if product as u32 >= range.wrapping_neg() % range {
                return (product >> 32) as usize;
            }
        }
    }
    pub fn shuffle<T>(&mut self, a: &mut [T]) {
        if a.is_empty() {
            return;
        }
        if u32::MAX as usize / a.len() >= a.len() {
            let mut i = 1;
            if a.len().is_multiple_of(2) {
                let j = self.below(2);
                a.swap(i, j);
                i += 1;
            }
            while i < a.len() {
                let n = i + 1;
                let x = self.below((n * (n + 1)) as u32);
                a.swap(i, x / (n + 1));
                a.swap(i + 1, x % (n + 1));
                i += 2;
            }
        } else {
            for i in 1..a.len() {
                let j = self.below((i + 1) as u32);
                a.swap(i, j);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mt19937_seed_zero_matches_reference_sequence() {
        let mut r = Rng::new();
        assert_eq!(
            [r.next(), r.next(), r.next(), r.next()],
            [2357136044, 2546248239, 3071714933, 3626093760]
        );
    }
}
