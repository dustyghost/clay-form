use std::sync::{Arc, Mutex};
use std::time::Instant;
#[derive(Clone)]
pub struct Tablet {
    pub state: Arc<Mutex<(f32, Instant, String)>>,
}
impl Tablet {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new((
            1.,
            Instant::now() - std::time::Duration::from_secs(10),
            "No Huion pressure device detected".into(),
        )));
        let result = Self {
            state: state.clone(),
        };
        #[cfg(target_os = "linux")]
        std::thread::spawn(move || {
            use std::io::Read;
            use std::os::unix::{fs::OpenOptionsExt, io::AsRawFd};
            loop {
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
                                    libc::ioctl(file.as_raw_fd(), 0x80184558u64, &mut abs)
                                };
                                if ok < 0 || abs[2] <= abs[1] {
                                    continue;
                                }
                                found = true;
                                state.lock().unwrap().2 = format!("Huion: {}", label.trim());
                                let mut buf = vec![0u8; std::mem::size_of::<libc::timeval>() + 8];
                                let mut pen_down = false;
                                loop {
                                    match file.read(&mut buf) {
                                        Ok(n) if n == buf.len() => {
                                            let o = buf.len() - 8;
                                            let kind = u16::from_ne_bytes([buf[o], buf[o + 1]]);
                                            let code = u16::from_ne_bytes([buf[o + 2], buf[o + 3]]);
                                            let value = i32::from_ne_bytes(
                                                buf[o + 4..o + 8].try_into().unwrap(),
                                            );
                                            if kind == 1 && code == 330 {
                                                pen_down = value != 0;
                                            }
                                            if kind == 3 && code == 24 {
                                                let mut s = state.lock().unwrap();
                                                s.0 = ((value - abs[1]) as f32
                                                    / (abs[2] - abs[1]) as f32)
                                                    .clamp(0., 1.);
                                                s.1 = Instant::now();
                                            }
                                        }
                                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                            if pen_down {
                                                state.lock().unwrap().1 = Instant::now();
                                            }
                                            std::thread::sleep(std::time::Duration::from_millis(8))
                                        }
                                        _ => {
                                            let mut s = state.lock().unwrap();
                                            s.1 =
                                                Instant::now() - std::time::Duration::from_secs(10);
                                            s.2 = "Huion disconnected; searching again".into();
                                            break;
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                state.lock().unwrap().2 = format!("Huion pressure unavailable: {e}")
                            }
                        }
                    }
                }
                if !found {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            }
        });
        result
    }
    pub fn pressure(&self) -> Option<f32> {
        let s = self.state.lock().unwrap();
        if s.1.elapsed().as_secs_f32() < 0.5 {
            Some(s.0)
        } else {
            None
        }
    }
    pub fn status(&self) -> String {
        self.state.lock().unwrap().2.clone()
    }
}
