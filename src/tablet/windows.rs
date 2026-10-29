//! Observe native pointer types before winit 0.30 erases PT_PEN vs PT_TOUCH.
//! Keep winit's dispatch, coordinate conversion, capture and pointer history intact.
use eframe::egui::{Event, RawInput, TouchPhase};
use std::{cell::RefCell, collections::BTreeSet};
use windows_sys::Win32::UI::{
    Input::Pointer::{
        GetPointerFrameInfoHistory, POINTER_FLAG_CANCELED, POINTER_FLAG_DOWN, POINTER_FLAG_UP,
        POINTER_FLAG_UPDATE, POINTER_INFO,
    },
    WindowsAndMessaging::{MSG, PT_PEN, WM_POINTERDOWN, WM_POINTERUP, WM_POINTERUPDATE},
};
use winit::platform::windows::EventLoopBuilderExtWindows;

struct Observation {
    id: u64,
    phase: TouchPhase,
    pen: bool,
    canceled: bool,
}
thread_local! {
    static OBSERVATIONS: RefCell<Vec<Observation>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn configure(options: &mut eframe::NativeOptions) {
    let previous = options.event_loop_builder.take();
    options.event_loop_builder = Some(Box::new(move |builder| {
        if let Some(previous) = previous {
            previous(builder);
        }
        builder.with_msg_hook(|message| {
            // SAFETY: winit passes a live Win32 MSG for the duration of this hook.
            let message = unsafe { &*message.cast::<MSG>() };
            if matches!(
                message.message,
                WM_POINTERDOWN | WM_POINTERUPDATE | WM_POINTERUP
            ) {
                observe(message.wParam as u32 & 0xffff);
            }
            // Never consume: winit must still dispatch and capture these pointers.
            false
        });
    }));
}

fn observe(id: u32) {
    let mut entries = 0;
    let mut pointers = 0;
    // SAFETY: the first call asks Windows for the required buffer dimensions.
    if unsafe { GetPointerFrameInfoHistory(id, &mut entries, &mut pointers, std::ptr::null_mut()) }
        == 0
    {
        return;
    }
    let Some(count) = entries.checked_mul(pointers) else {
        return;
    };
    let mut history: Vec<POINTER_INFO> = Vec::with_capacity(count as usize);
    // SAFETY: the buffer has capacity for the dimensions returned by Windows.
    // Windows only writes to it during this synchronous call on the UI thread.
    if unsafe { GetPointerFrameInfoHistory(id, &mut entries, &mut pointers, history.as_mut_ptr()) }
        == 0
    {
        return;
    }
    let count = (entries as usize).saturating_mul(pointers as usize);
    if count > history.capacity() {
        return;
    }
    // SAFETY: the successful call initialized entries * pointers records.
    unsafe { history.set_len(count) };
    OBSERVATIONS.with_borrow_mut(|observations| {
        // Match winit's reverse traversal of newest-first Win32 history exactly.
        for info in history.iter().rev() {
            let phase = if info.pointerFlags & POINTER_FLAG_DOWN != 0 {
                TouchPhase::Start
            } else if info.pointerFlags & POINTER_FLAG_UP != 0 {
                TouchPhase::End
            } else if info.pointerFlags & POINTER_FLAG_UPDATE != 0 {
                TouchPhase::Move
            } else {
                continue;
            };
            observations.push(Observation {
                id: info.pointerId as u64,
                phase,
                pen: info.pointerType == PT_PEN,
                canceled: info.pointerFlags & POINTER_FLAG_CANCELED != 0,
            });
        }
    });
}

pub(super) fn pen_contacts(input: &mut RawInput) -> BTreeSet<u64> {
    let mut observations = OBSERVATIONS.with_borrow_mut(std::mem::take);
    let mut pens = BTreeSet::new();
    for event in &mut input.events {
        if let Event::Touch { id, phase, .. } = event
            && let Some(index) = observations
                .iter()
                .position(|o| o.id == id.0 && o.phase == *phase)
        {
            let observation = observations.remove(index);
            if observation.pen {
                pens.insert(id.0);
            }
            if observation.canceled {
                *phase = TouchPhase::Cancel;
            }
        }
    }
    pens
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coalesced_pen_and_finger_contacts_keep_their_native_type() {
        OBSERVATIONS.with_borrow_mut(|observations| {
            for (id, pen, phase, canceled) in [
                (1, false, TouchPhase::Start, false),
                (2, true, TouchPhase::Start, false),
                (2, true, TouchPhase::Move, true),
            ] {
                observations.push(Observation {
                    id,
                    pen,
                    phase,
                    canceled,
                });
            }
        });
        let mut input = RawInput::default();
        for (id, phase) in [
            (1, TouchPhase::Start),
            (2, TouchPhase::Start),
            (2, TouchPhase::Move),
        ] {
            input.events.push(Event::Touch {
                device_id: eframe::egui::TouchDeviceId(0),
                id: eframe::egui::TouchId(id),
                phase,
                pos: eframe::egui::Pos2::ZERO,
                force: Some(0.5),
            });
        }
        assert_eq!(pen_contacts(&mut input), BTreeSet::from([2]));
        assert!(matches!(
            input.events[2],
            Event::Touch {
                phase: TouchPhase::Cancel,
                ..
            }
        ));
        // An ended contact's old classification must not tag a reused pointer ID.
        assert!(pen_contacts(&mut input).is_empty());
    }
}
