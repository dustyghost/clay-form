use super::Material;
use std::sync::Arc;

impl Material {
    /// Compare snapshot contents exactly, including signed zero and NaN bits.
    /// Shared immutable layer/link payloads need at most one value comparison
    /// per consecutive pair of allocations; no identities outlive this call.
    pub(crate) fn slices_bitwise_equal(left: &[Self], right: &[Self]) -> bool {
        if left.len() != right.len() {
            return false;
        }
        let mut layers = None;
        let mut links = None;
        for (a, b) in left.iter().zip(right) {
            if a.surface().values().map(f32::to_bits) != b.surface().values().map(f32::to_bits) {
                return false;
            }
            let layer_pair = (a.layers.storage_id(), b.layers.storage_id());
            if layer_pair.0 != layer_pair.1 && layers != Some(layer_pair) {
                if !a.layers.iter().zip(b.layers.iter()).all(|(a, b)| {
                    a.surface.values().map(f32::to_bits) == b.surface.values().map(f32::to_bits)
                        && a.weight.to_bits() == b.weight.to_bits()
                        && a.add.to_bits() == b.add.to_bits()
                }) {
                    return false;
                }
                layers = Some(layer_pair);
            }
            let link_pair = (Arc::as_ptr(&a.links), Arc::as_ptr(&b.links));
            if link_pair.0 != link_pair.1 && links != Some(link_pair) {
                if a.links.len() != b.links.len()
                    || !a.links.iter().zip(b.links.iter()).all(|(a, b)| {
                        a.id == b.id && a.weights.map(f32::to_bits) == b.weights.map(f32::to_bits)
                    })
                {
                    return false;
                }
                links = Some(link_pair);
            }
        }
        true
    }

    /// Previous numeric-equality path retained as a regression reference.
    #[cfg(test)]
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

    #[test]
    fn snapshot_material_equality_checks_bits_in_all_channels_layers_and_links() {
        let mut original = Material::default().with_link(7);
        original.layers[0].weight = 0.5;
        assert!(Material::slices_bitwise_equal(&[], &[]));
        assert!(!Material::slices_bitwise_equal(
            std::slice::from_ref(&original),
            &[]
        ));
        for channel in 0..VALUE_COUNT {
            let mut values = original.values();
            // Toggle a bit rather than adding: covers signed zero as well as
            // every base, layer, weight and blend channel.
            values[channel] = f32::from_bits(values[channel].to_bits() ^ 0x80000000);
            let changed = Material::from_values(values).with_link(7);
            assert!(
                !Material::slices_bitwise_equal(std::slice::from_ref(&original), &[changed]),
                "channel {channel}"
            );
        }
        for field in 0..5 {
            let mut changed = original.clone();
            let link = &mut Arc::make_mut(&mut changed.links)[0];
            if field == 0 {
                link.id += 1;
            } else {
                link.weights[field - 1] = -0.;
            }
            assert!(!Material::slices_bitwise_equal(
                std::slice::from_ref(&original),
                &[changed]
            ));
        }
        let mut nan = original.clone();
        nan.layers[0].surface.emission[0] = f32::from_bits(0x7fc00001);
        let mut distinct = nan.clone();
        // Force separate copy-on-write storage with identical bits.
        distinct.layers[0].surface.emission[0] = f32::from_bits(0x7fc00001);
        assert!(Material::slices_bitwise_equal(
            std::slice::from_ref(&nan),
            std::slice::from_ref(&distinct)
        ));
        distinct.layers[0].surface.emission[0] = f32::from_bits(0x7fc00002);
        assert!(!Material::slices_bitwise_equal(&[nan], &[distinct]));
    }

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
