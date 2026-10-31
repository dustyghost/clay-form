//! Compute complete brush normals only where a stamp can read them.
use super::{Dab, Kind, Mesh, Vec3};

impl Mesh {
    pub(super) fn stamp_normals(
        &self,
        kind: Kind,
        dabs: &[Dab],
        sculptable: &mut [bool],
    ) -> Vec<Vec3> {
        #[cfg(test)]
        if tests::FULL_NORMALS.get() {
            return self.normals();
        }
        // Polish compares every face to its vertex normals when protecting corners.
        if kind == Kind::Polish {
            return self.normals();
        }
        // A widely covering brush gains nothing from a complete selection pass.
        // Sampling only chooses the full path: an unsampled outlier still gets
        // its exact normal and the stamp loop's usual distance rejection.
        if self
            .vertices
            .iter()
            .zip(sculptable.iter())
            .step_by((self.vertices.len() / 64).max(1))
            .all(|(&point, &visible)| {
                visible
                    && !dabs
                        .iter()
                        .all(|dab| (point - dab.center).len() > dab.radius * 1.5)
            })
        {
            return if kind == Kind::Snake {
                Vec::new()
            } else {
                self.normals()
            };
        }
        let mut count = 0;
        for (&point, needed) in self.vertices.iter().zip(sculptable.iter_mut()) {
            // Use the stamp loop's exact rejection test, including its boundary.
            if *needed
                && dabs
                    .iter()
                    .all(|dab| (point - dab.center).len() > dab.radius * 1.5)
            {
                *needed = false;
            }
            count += usize::from(*needed);
        }
        // Snake never uses surface normals: its motion and frame come from the dab.
        if kind == Kind::Snake || count == 0 {
            return Vec::new();
        }
        if count == self.vertices.len() {
            return self.normals();
        }
        self.normals_for(sculptable)
    }

    /// Selected entries include every incident triangle, in the original summation order.
    /// Other entries are scratch values and must not be read as complete normals.
    fn normals_for(&self, needed: &[bool]) -> Vec<Vec3> {
        let mut normals = vec![Vec3::default(); self.vertices.len()];
        for &[a, b, c] in &self.triangles {
            if !needed[a] && !needed[b] && !needed[c] {
                continue;
            }
            let normal =
                (self.vertices[b] - self.vertices[a]).cross(self.vertices[c] - self.vertices[a]);
            for i in [a, b, c] {
                normals[i] = normals[i] + normal;
            }
        }
        for (normal, &needed) in normals.iter_mut().zip(needed) {
            if needed {
                *normal = normal.norm();
            }
        }
        normals
    }
}

#[cfg(test)]
mod tests;
