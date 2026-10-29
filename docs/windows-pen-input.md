# Windows pen input verification

Clay Form observes the native pointer frame before winit 0.30 converts both
`PT_PEN` and `PT_TOUCH` into `Touch`. The message hook returns false so winit
still handles dispatch, capture, coordinate conversion and coalesced history.
The raw-input adapter removes pen contacts from touch gestures, reads pressure
only from those pen contacts, and synthesizes a pen-owned primary pointer.
It suppresses emulated finger/mouse pointer events during pen input and retains
suppression for existing fingers until they lift. A takeover clears any camera
gesture already in progress. No pressure threshold is used to identify a pen.

Focused automated checks: `cargo test --locked --lib tablet::`. Windows PR
packaging CI runs these checks on both Windows targets, including the native
classification/cancellation test. The portable routing tests also run on Linux.

Check on a Windows touchscreen with an active pen before accepting hardware
support:

1. Enable touch gestures and use the default one-finger orbit mapping. Drag a
   finger to rotate, then two fingers to pan/pinch. Lift all contacts.
2. Hover the pen: the brush cursor follows without sculpting. Press and drag:
   clay changes while the camera remains still. Lift and repeat several times.
3. Enable brush strength pressure and compare light and firm strokes. Confirm
   the live pressure changes; a finger must not supply brush pressure.
4. Rest a finger first, then start a pen stroke. The pen must own the stroke
   position; moving or lifting that finger must not move the camera, change
   pressure, end the pen stroke or trigger undo/redo.
5. Repeat with a palm/finger added after the pen. After lifting the pen, existing
   fingers remain ignored until lifted; a fresh finger gesture works normally.
6. Test a quick tap, drag outside the client area, canceled contact, and switching
   away from the app mid-stroke. Return and confirm no stuck stroke or gesture.
7. Repeat pen sculpting with touch gestures disabled. Check mouse controls and
   ordinary UI buttons after all pen/finger contacts have ended.

Physical-device verification is required in addition to passing CI; synthetic
input cannot establish what a particular driver delivers.
