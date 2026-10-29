//! Pair native pointer metadata with egui events across reentrant UI updates.
use eframe::egui::{Event, RawInput, TouchPhase};
use std::collections::{BTreeSet, VecDeque};

struct Observation {
    id: u64,
    phase: TouchPhase,
    pen: bool,
    canceled: bool,
}

#[derive(Default)]
pub(super) struct ContactTypes {
    pending: VecDeque<Observation>,
    native_pen: u64,
    native_finger: u64,
    routed_pen: u64,
    routed_finger: u64,
    unmatched: u64,
    discarded: u64,
}

impl ContactTypes {
    pub fn record(&mut self, id: u64, phase: TouchPhase, pen: bool, canceled: bool) {
        if pen {
            self.native_pen += 1;
        } else {
            self.native_finger += 1;
        }
        // Orphaned metadata is possible if winit cannot convert an input record.
        // Bound it without clearing valid metadata on unrelated repaint passes.
        if self.pending.len() == 16384 {
            self.pending.pop_front();
            self.discarded += 1;
        }
        self.pending.push_back(Observation {
            id,
            phase,
            pen,
            canceled,
        });
    }

    pub fn classify(&mut self, input: &mut RawInput) -> BTreeSet<u64> {
        let observations = &mut self.pending;
        let mut pens = BTreeSet::new();
        for event in &mut input.events {
            if let Event::Touch { id, phase, .. } = event {
                if let Some(index) = observations
                    .iter()
                    .position(|o| o.id == id.0 && o.phase == *phase)
                {
                    let observation = observations.remove(index).unwrap();
                    if observation.pen {
                        pens.insert(id.0);
                        self.routed_pen += 1;
                    } else {
                        self.routed_finger += 1;
                    }
                    if observation.canceled {
                        *phase = TouchPhase::Cancel;
                    }
                } else {
                    self.unmatched += 1;
                }
            }
        }
        pens
    }

    pub fn status(&self) -> String {
        format!(
            "Windows input: native pen {}, routed pen {}; native finger {}, routed finger {}; unclassified {}; pending {}; discarded {}",
            self.native_pen,
            self.routed_pen,
            self.native_finger,
            self.routed_finger,
            self.unmatched,
            self.pending.len(),
            self.discarded
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(id: u64, phase: TouchPhase) -> RawInput {
        RawInput {
            events: vec![Event::Touch {
                device_id: eframe::egui::TouchDeviceId(0),
                id: eframe::egui::TouchId(id),
                phase,
                pos: eframe::egui::Pos2::ZERO,
                force: Some(0.5),
            }],
            ..Default::default()
        }
    }
    #[test]
    fn native_metadata_survives_an_unrelated_repaint_before_touch_dispatch() {
        let mut types = ContactTypes::default();
        types.record(2, TouchPhase::Start, true, false);
        assert!(types.classify(&mut RawInput::default()).is_empty());
        assert_eq!(
            types.classify(&mut input(2, TouchPhase::Start)),
            BTreeSet::from([2])
        );
    }
    #[test]
    fn coalesced_frame_can_be_delivered_in_separate_ui_updates() {
        let mut types = ContactTypes::default();
        types.record(1, TouchPhase::Start, false, false);
        types.record(2, TouchPhase::Start, true, false);
        types.record(2, TouchPhase::Move, true, true);
        assert!(types.classify(&mut input(1, TouchPhase::Start)).is_empty());
        assert_eq!(
            types.classify(&mut input(2, TouchPhase::Start)),
            BTreeSet::from([2])
        );
        let mut canceled = input(2, TouchPhase::Move);
        assert_eq!(types.classify(&mut canceled), BTreeSet::from([2]));
        assert!(matches!(
            canceled.events[0],
            Event::Touch {
                phase: TouchPhase::Cancel,
                ..
            }
        ));
        assert!(types.pending.is_empty());
        // No old metadata should tag a reused ID as a pen.
        assert!(types.classify(&mut input(2, TouchPhase::Start)).is_empty());
    }
    #[test]
    fn unconverted_records_are_bounded_and_reported() {
        let mut types = ContactTypes::default();
        for id in 0..16400 {
            types.record(id, TouchPhase::Start, true, false);
        }
        assert_eq!(types.pending.len(), 16384);
        assert_eq!(types.discarded, 16);
        assert!(types.status().contains("discarded 16"));
    }
}
