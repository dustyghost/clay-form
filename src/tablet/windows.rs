//! Observe native pointer types before winit 0.30 erases PT_PEN vs PT_TOUCH.
//! Keep winit's dispatch, coordinate conversion, capture and pointer history intact.
use super::contact_types::ContactTypes;
use eframe::egui::{RawInput, TouchPhase};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
};
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::UI::{
    Input::Pointer::{
        GetPointerFrameInfoHistory, POINTER_FLAG_CANCELED, POINTER_FLAG_DOWN, POINTER_FLAG_UP,
        POINTER_FLAG_UPDATE, POINTER_INFO,
    },
    WindowsAndMessaging::{MSG, PT_PEN, WM_POINTERDOWN, WM_POINTERUP, WM_POINTERUPDATE},
};
use winit::platform::windows::EventLoopBuilderExtWindows;

thread_local! {
    static OBSERVATIONS: RefCell<ContactTypes> = RefCell::default();
    static LAST_ERROR: Cell<u32> = const { Cell::new(0) };
    static API_FAILURES: Cell<u64> = const { Cell::new(0) };
}

pub(super) fn status() -> String {
    OBSERVATIONS.with_borrow(|observations| {
        format!(
            "{}; native API failures {}, last error {}",
            observations.status(),
            API_FAILURES.get(),
            LAST_ERROR.get()
        )
    })
}

fn record_api_error() {
    // SAFETY: GetLastError reads this UI thread's last Win32 error.
    LAST_ERROR.set(unsafe { GetLastError() });
    API_FAILURES.set(API_FAILURES.get() + 1);
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
        record_api_error();
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
        record_api_error();
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
            observations.record(
                info.pointerId as u64,
                phase,
                info.pointerType == PT_PEN,
                info.pointerFlags & POINTER_FLAG_CANCELED != 0,
            );
        }
    });
}

pub(super) fn pen_contacts(input: &mut RawInput) -> BTreeSet<u64> {
    // eframe may repaint reentrantly while winit dispatches a native frame.
    // Remove only metadata whose corresponding touch event actually arrived.
    OBSERVATIONS.with_borrow_mut(|observations| observations.classify(input))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Event;
    #[test]
    fn coalesced_pen_and_finger_contacts_keep_their_native_type() {
        OBSERVATIONS.with_borrow_mut(|observations| {
            for (id, pen, phase, canceled) in [
                (1, false, TouchPhase::Start, false),
                (2, true, TouchPhase::Start, false),
                (2, true, TouchPhase::Move, true),
            ] {
                observations.record(id, phase, pen, canceled);
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
