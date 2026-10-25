//! Observe all pressure-capable pen endpoints without grabbing the desktop device.
use super::SharedState;
use std::{
    io::Read,
    os::unix::{fs::OpenOptionsExt, io::AsRawFd},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
const EV_KEY: u16 = 1;
const EV_ABS: u16 = 3;
const ABS_PRESSURE: u16 = 24;
const BTN_TOUCH: u16 = 330;
const EVIOCGABS_PRESSURE: libc::c_ulong = 0x80184558;

fn capability(bits: &str, bit: usize) -> bool {
    let width = usize::BITS as usize;
    bits.split_whitespace()
        .rev()
        .nth(bit / width)
        .and_then(|word| usize::from_str_radix(word, 16).ok())
        .is_some_and(|word| word & (1 << (bit % width)) != 0)
}
fn pen_capabilities(abs: &str, keys: &str) -> bool {
    capability(abs, ABS_PRESSURE as usize)
        && (capability(keys, 320) || capability(keys, 321))
        && capability(keys, BTN_TOUCH as usize)
}
#[derive(Default)]
struct PenSample {
    pressure: f32,
    down: bool,
}
impl PenSample {
    fn event(&mut self, kind: u16, code: u16, value: i32, range: [i32; 2]) {
        if kind == EV_KEY && code == BTN_TOUCH {
            self.down = value != 0;
        }
        if kind == EV_ABS && code == ABS_PRESSURE {
            self.pressure = ((value as f64 - range[0] as f64) / (range[1] as f64 - range[0] as f64))
                .clamp(0., 1.) as f32;
        }
    }
}
struct Device {
    path: PathBuf,
    label: String,
    file: std::fs::File,
    range: [i32; 2],
    sample: PenSample,
    updated: Instant,
    dropped: bool,
}
fn pressure_info(file: &std::fs::File) -> Option<[i32; 6]> {
    let mut abs = [0i32; 6];
    // EVIOCGABS writes precisely six i32 fields into this buffer.
    let ok = unsafe { libc::ioctl(file.as_raw_fd(), EVIOCGABS_PRESSURE, &mut abs) };
    (ok >= 0 && abs[2] > abs[1]).then_some(abs)
}
fn contact(file: &std::fs::File) -> Option<bool> {
    let mut keys = [0u8; 96];
    // EVIOCGKEY(96): read the key bitmap, inspect only BTN_TOUCH.
    let ok = unsafe { libc::ioctl(file.as_raw_fd(), 0x80604518 as libc::c_ulong, &mut keys) };
    (ok >= 0).then_some(keys[BTN_TOUCH as usize / 8] & (1 << (BTN_TOUCH % 8)) != 0)
}
fn discover(devices: &mut Vec<Device>) -> String {
    let mut errors = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/sys/class/input") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if !name.to_string_lossy().starts_with("event") {
                continue;
            }
            let base = entry.path().join("device");
            let abs = std::fs::read_to_string(base.join("capabilities/abs")).unwrap_or_default();
            let keys = std::fs::read_to_string(base.join("capabilities/key")).unwrap_or_default();
            if !pen_capabilities(&abs, &keys) {
                continue;
            }
            let path = PathBuf::from("/dev/input").join(name);
            if devices.iter().any(|d| d.path == path) {
                continue;
            }
            let label = std::fs::read_to_string(base.join("name"))
                .unwrap_or_else(|_| "Pen tablet".into())
                .trim()
                .to_owned();
            match std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(&path)
            {
                Ok(file) => {
                    let Some(abs) = pressure_info(&file) else {
                        continue;
                    };
                    let mut sample = PenSample::default();
                    sample.event(EV_ABS, ABS_PRESSURE, abs[0], [abs[1], abs[2]]);
                    sample.down = contact(&file).unwrap_or(false);
                    devices.push(Device {
                        path,
                        label,
                        file,
                        range: [abs[1], abs[2]],
                        sample,
                        updated: Instant::now(),
                        dropped: false,
                    });
                }
                Err(e) => errors.push(format!("{label}: {e}")),
            }
        }
    }
    let mut status = if devices.is_empty() {
        "No accessible pen pressure device".into()
    } else {
        format!(
            "Tablet: {}",
            devices
                .iter()
                .map(|d| d.label.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        )
    };
    if !errors.is_empty() {
        status.push_str(&format!(". Unavailable: {}", errors.join("; ")));
    }
    status
}
pub(super) fn spawn(state: SharedState, stop: Arc<AtomicBool>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut devices = Vec::new();
        let mut scan = Instant::now() - Duration::from_secs(3);
        let mut status = String::new();
        let mut buffer = vec![0u8; std::mem::size_of::<libc::timeval>() + 8];
        while !stop.load(Ordering::Relaxed) {
            if scan.elapsed() >= Duration::from_secs(2) {
                status = discover(&mut devices);
                scan = Instant::now();
            }
            devices.retain_mut(|device| {
                for _ in 0..1024 {
                    match device.file.read(&mut buffer) {
                        Ok(n) if n == buffer.len() => {
                            let o = n - 8;
                            let kind = u16::from_ne_bytes([buffer[o], buffer[o + 1]]);
                            let code = u16::from_ne_bytes([buffer[o + 2], buffer[o + 3]]);
                            let value =
                                i32::from_ne_bytes(buffer[o + 4..o + 8].try_into().unwrap());
                            if kind == 0 && code == 3 {
                                // Ignore the lost packet through the next SYN_REPORT.
                                device.dropped = true;
                                continue;
                            }
                            if device.dropped {
                                if kind != 0 || code != 0 {
                                    continue;
                                }
                                let Some(abs) = pressure_info(&device.file) else {
                                    return false;
                                };
                                device
                                    .sample
                                    .event(EV_ABS, ABS_PRESSURE, abs[0], device.range);
                                let Some(down) = contact(&device.file) else {
                                    return false;
                                };
                                device.sample.down = down;
                                device.dropped = false;
                                device.updated = Instant::now();
                            } else {
                                device.sample.event(kind, code, value, device.range);
                                if kind == EV_ABS && code == ABS_PRESSURE
                                    || kind == EV_KEY && code == BTN_TOUCH
                                {
                                    device.updated = Instant::now();
                                }
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        _ => {
                            scan = Instant::now() - Duration::from_secs(3);
                            return false;
                        }
                    }
                }
                true
            });
            let mut shared = state.lock().unwrap();
            if let Some(device) = devices
                .iter()
                .filter(|d| d.sample.down && !d.dropped)
                .max_by_key(|d| d.updated)
            {
                shared.pressure = device.sample.pressure;
                // Holding pressure need not produce further kernel events.
                shared.updated_at = Instant::now();
            } else {
                shared.updated_at = Instant::now() - Duration::from_secs(10);
            }
            shared.status.clone_from(&status);
            drop(shared);
            std::thread::sleep(Duration::from_millis(8));
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_physical_and_virtual_pens_without_selecting_keyboards() {
        assert!(pen_capabilities("d000003", "c03 0 0 0 0 0"));
        assert!(pen_capabilities("d000003", "1e03 0 0 0 0 0"));
        assert!(!pen_capabilities("100000000", "ffffffffffffffff"));
        assert!(!pen_capabilities("3", "c03 0 0 0 0 0"));
    }
    #[test]
    fn normalized_strength_keeps_held_pressure_and_releases_contact() {
        let mut sample = PenSample::default();
        sample.event(EV_KEY, BTN_TOUCH, 1, [100, 1100]);
        sample.event(EV_ABS, ABS_PRESSURE, 350, [100, 1100]);
        assert!(sample.down);
        assert_eq!(sample.pressure, 0.25);
        sample.event(0, 0, 0, [100, 1100]);
        assert_eq!(sample.pressure, 0.25);
        sample.event(EV_ABS, ABS_PRESSURE, 850, [100, 1100]);
        assert_eq!(sample.pressure, 0.75);
        sample.event(EV_KEY, BTN_TOUCH, 0, [100, 1100]);
        assert!(!sample.down);
    }
}
