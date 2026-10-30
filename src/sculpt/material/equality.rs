use super::Material;
use std::sync::Arc;

impl Material {
    /// Exact slice equality, reusing successful comparisons of shared payloads.
    pub(crate) fn slices_equal(left: &[Self], right: &[Self]) -> bool {
        if left.len() != right.len() {
            return false;
        }
        let mut layers = None;
        let mut links = None;
        for (a, b) in left.iter().zip(right) {
            if a.surface() != b.surface() {
                return false;
            }
            let layer_pair = (a.layers.storage_id(), b.layers.storage_id());
            if layers != Some(layer_pair) {
                // Pointer identity alone is insufficient: a shared payload can
                // contain NaN. Only remember a successful value comparison.
                if a.layers != b.layers {
                    return false;
                }
                layers = Some(layer_pair);
            }
            let link_pair = (Arc::as_ptr(&a.links), Arc::as_ptr(&b.links));
            if links != Some(link_pair) {
                if a.links != b.links {
                    return false;
                }
                links = Some(link_pair);
            }
        }
        // The borrowed slices keep every payload alive and immutable throughout
        // this scan. No identities or equality results survive the call.
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::material::layers::{Influence, VALUE_COUNT, limits};

    fn check(left: &[Material], right: &[Material], expected: bool) {
        assert_eq!(left == right, expected);
        assert_eq!(Material::slices_equal(left, right), expected);
        assert_eq!(Material::slices_equal(right, left), expected);
    }

    #[test]
    fn material_slice_equality_checks_every_surface_layer_and_link_value() {
        let material = Material::default().with_link(7);
        let original = vec![material.clone(); 8];
        check(&[], &[], true);
        check(&original, &original[..7], false);
        check(&original, &original.clone(), true);
        for channel in 0..VALUE_COUNT {
            let mut values = material.values();
            let (lo, hi) = limits(channel);
            values[channel] = if values[channel] == lo { hi } else { lo };
            let mut changed = original.clone();
            changed[7] = Material::from_values(values).with_link(7);
            check(&original, &changed, false);
        }
        for field in 0..6 {
            let mut changed = original.clone();
            let links = Arc::make_mut(&mut changed[7].links);
            match field {
                0 => links[0].id += 1,
                1..=4 => links[0].weights[field - 1] = 0.5,
                _ => links.clear(),
            }
            check(&original, &changed, false);
        }
    }

    #[test]
    fn material_slice_equality_handles_distinct_storage_and_copy_on_write() {
        let mut a = Material::default().with_link(7);
        a.layers[0].weight = 0.5;
        let b = Material::from_values(a.values()).with_link(7);
        assert_ne!(a.layers.storage_id(), b.layers.storage_id());
        assert!(!Arc::ptr_eq(&a.links, &b.links));
        let left = [a.clone(), a.clone(), b.clone(), a.clone(), b.clone()];
        let mut right = [b.clone(), b, a.clone(), a.clone(), a];
        check(&left, &right, true);
        right[4].layers[2].surface.roughness = 0.9;
        check(&left, &right, false);
        right[4] = left[4].clone();
        check(&left, &right, true);
        Arc::make_mut(&mut right[4].links)[0].weights[3] = 0.;
        check(&left, &right, false);
    }

    #[test]
    fn material_slice_equality_preserves_float_semantics_in_shared_payloads() {
        let mut positive = Material {
            links: Arc::new(vec![Influence {
                id: 7,
                weights: [0.; 4],
            }]),
            ..Default::default()
        };
        let mut negative = positive.clone();
        negative.metallic = -0.;
        negative.layers[2].add = -0.;
        Arc::make_mut(&mut negative.links)[0].weights[2] = -0.;
        check(&vec![positive.clone(); 4], &vec![negative; 4], true);
        for field in 0..3 {
            let mut invalid = positive.clone();
            match field {
                0 => invalid.roughness = f32::NAN,
                1 => invalid.layers[2].surface.roughness = f32::NAN,
                _ => Arc::make_mut(&mut invalid.links)[0].weights[2] = f32::NAN,
            }
            // A valid repeated pair precedes the non-reflexive shared payload.
            let mixed = [positive.clone(), positive.clone(), invalid];
            check(&mixed, &mixed.clone(), false);
        }
        positive.emission_strength = f32::INFINITY;
        check(&[positive.clone()], &[positive], true);
    }
}
