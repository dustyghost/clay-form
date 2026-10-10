//! Linux evdev adapter. It observes Huion events without grabbing the device.
use super::SharedState;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
const EV_KEY: u16 = 1;
const EV_ABS: u16 = 3;
const ABS_PRESSURE: u16 = 24;
const BTN_TOUCH: u16 = 330;
// _IOR('E', 0x40 + ABS_PRESSURE, struct input_absinfo), six i32 fields.
const EVIOCGABS_PRESSURE: libc::c_ulong = 0x80184558;
pub(super) fn spawn(state: SharedState, stop: Arc<AtomicBool>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        use std::io::Read;
        use std::os::unix::{fs::OpenOptionsExt, io::AsRawFd};
        while !stop.load(Ordering::Relaxed) {
            let mut found = false;
            if let Ok(entries) = std::fs::read_dir("/sys/class/input") {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !name.starts_with("event") {
                        continue;
                    }
                    let label = std::fs::read_to_string(entry.path().join("device/name"))
                        .unwrap_or_default();
                    if !label.to_lowercase().contains("huion") {
                        continue;
                    }
                    let path = format!("/dev/input/{name}");
                    match std::fs::OpenOptions::new()
                        .read(true)
                        .custom_flags(libc::O_NONBLOCK)
                        .open(&path)
                    {
                        Ok(mut file) => {
                            // Linux EVIOCGABS(ABS_PRESSURE): kernel writes six i32 values into this buffer.
                            let mut abs = [0i32; 6];
                            let ok = unsafe {
                                libc::ioctl(file.as_raw_fd(), EVIOCGABS_PRESSURE, &mut abs)
                            };
                            if ok < 0 || abs[2] <= abs[1] {
                                continue;
                            }
                            found = true;
                            state.lock().unwrap().status = format!("Huion: {}", label.trim());
                            let mut buf = vec![0u8; std::mem::size_of::<libc::timeval>() + 8];
                            let mut pen_down = false;
                            while !stop.load(Ordering::Relaxed) {
                                match file.read(&mut buf) {
                                    Ok(n) if n == buf.len() => {
                                        let o = buf.len() - 8;
                                        let kind = u16::from_ne_bytes([buf[o], buf[o + 1]]);
                                        let code = u16::from_ne_bytes([buf[o + 2], buf[o + 3]]);
                                        let value = i32::from_ne_bytes(
                                            buf[o + 4..o + 8].try_into().unwrap(),
                                        );
                                        if kind == EV_KEY && code == BTN_TOUCH {
                                            pen_down = value != 0;
                                        }
                                        if kind == EV_ABS && code == ABS_PRESSURE {
                                            let mut s = state.lock().unwrap();
                                            s.pressure = ((value - abs[1]) as f32
                                                / (abs[2] - abs[1]) as f32)
                                                .clamp(0., 1.);
                                            s.updated_at = Instant::now();
                                        }
                                    }
                                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                        if pen_down {
                                            state.lock().unwrap().updated_at = Instant::now();
                                        }
                                        std::thread::sleep(std::time::Duration::from_millis(8))
                                    }
                                    _ => {
                                        let mut s = state.lock().unwrap();
                                        s.updated_at =
                                            Instant::now() - std::time::Duration::from_secs(10);
                                        s.status = "Huion disconnected; searching again".into();
                                        break;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            state.lock().unwrap().status =
                                format!("Huion pressure unavailable on {path}: {e}")
                        }
                    }
                }
            }
            if !found {
                for _ in 0..20 {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }
    })
}
