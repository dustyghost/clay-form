//! Independent, atomic recovery snapshots. One worker orders writes and retirement,
//! so an in-flight write cannot resurrect a saved or deliberately discarded session.
use crate::{persistence, sculpt::Project};
use serde::{Deserialize, Serialize};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread::JoinHandle,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Settings {
    pub enabled: bool,
    pub interval_minutes: u32,
    pub versions: usize,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_minutes: 2,
            versions: 5,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=60).contains(&self.interval_minutes) || !(1..=20).contains(&self.versions) {
            return Err(
                "Recovery interval must be 1–60 minutes and retention 1–20 versions".into(),
            );
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
struct Snapshot {
    version: u32,
    source_path: String,
    project: Project,
}
#[derive(Clone)]
pub(crate) struct Entry {
    pub path: PathBuf,
    pub modified: SystemTime,
    session: String,
}
impl Entry {
    pub fn age_label(&self) -> String {
        let seconds = self.modified.elapsed().unwrap_or_default().as_secs();
        if seconds < 60 {
            "Less than a minute ago".into()
        } else if seconds < 3600 {
            format!("{} minutes ago", seconds / 60)
        } else if seconds < 86400 {
            format!("{} hours ago", seconds / 3600)
        } else {
            format!("{} days ago", seconds / 86400)
        }
    }
}
pub(crate) fn entries(directory: &Path) -> Result<Vec<Entry>, String> {
    let read = match std::fs::read_dir(directory) {
        Ok(read) => read,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut entries = Vec::new();
    for item in read {
        let item = item.map_err(|e| e.to_string())?;
        let name = item.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(stem) = name
            .strip_prefix("recovery-")
            .and_then(|s| s.strip_suffix(".json"))
        else {
            continue;
        };
        let Some((session, sequence)) = stem.rsplit_once('-') else {
            continue;
        };
        if session.len() != 41
            || sequence.len() != 8
            || !stem.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
        {
            continue;
        }
        if !item.file_type().map_err(|e| e.to_string())?.is_file() {
            continue;
        }
        let modified = item
            .metadata()
            .and_then(|m| m.modified())
            .map_err(|e| e.to_string())?;
        entries.push(Entry {
            path: item.path(),
            modified,
            session: session.into(),
        });
    }
    entries.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| b.path.cmp(&a.path))
    });
    Ok(entries)
}
pub(crate) fn load(entry: &Entry) -> Result<(Project, String), String> {
    let snapshot: Snapshot = persistence::read_json(&entry.path.to_string_lossy())?;
    if snapshot.version != 1 {
        return Err("Unsupported recovery version".into());
    }
    snapshot.project.validate()?;
    Ok((snapshot.project, snapshot.source_path))
}

enum Job {
    Write {
        session: String,
        sequence: u32,
        source: String,
        project: Box<Project>,
        versions: usize,
    },
    Retire(String),
}
pub(crate) struct Completion {
    pub written: bool,
    pub result: Result<(), String>,
}
pub(crate) struct Recovery {
    pub directory: PathBuf,
    session: String,
    recovered_session: Option<String>,
    sequence: u32,
    sender: Option<Sender<Job>>,
    results: Receiver<(bool, Result<bool, String>)>,
    worker: Option<JoinHandle<()>>,
    pub busy: bool,
    last_attempt: Instant,
}
impl Recovery {
    pub fn new(directory: PathBuf) -> Result<Self, String> {
        let (sender, jobs) = mpsc::channel();
        let (results_sender, results) = mpsc::channel();
        let worker_directory = directory.clone();
        let worker = std::thread::Builder::new()
            .name("clay-recovery".into())
            .spawn(move || {
                let mut previous: Option<(String, u64)> = None;
                for job in jobs {
                    let writing = matches!(job, Job::Write { .. });
                    let result = match job {
                        Job::Write {
                            session,
                            sequence,
                            source,
                            project,
                            versions,
                        } => write(
                            &worker_directory,
                            &session,
                            sequence,
                            source,
                            *project,
                            versions,
                            &mut previous,
                        ),
                        Job::Retire(session) => {
                            if previous.as_ref().is_some_and(|(s, _)| *s == session) {
                                previous = None;
                            }
                            retire(&worker_directory, &session).map(|()| false)
                        }
                    };
                    // Retirement errors are also visible, but don't release an unrelated write.
                    let _ = results_sender.send((writing, result));
                }
            })
            .map_err(|e| e.to_string())?;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Ok(Self {
            directory,
            session: format!("{nanos:032x}-{:08x}", std::process::id()),
            recovered_session: None,
            sequence: 0,
            sender: Some(sender),
            results,
            worker: Some(worker),
            busy: false,
            last_attempt: Instant::now(),
        })
    }
    pub fn due(&self, settings: &Settings) -> bool {
        settings.enabled
            && !self.busy
            && self.last_attempt.elapsed()
                >= Duration::from_secs(u64::from(settings.interval_minutes) * 60)
    }
    pub fn save(
        &mut self,
        project: Project,
        source: String,
        versions: usize,
    ) -> Result<(), String> {
        if self.busy {
            return Err("A recovery copy is already being written".into());
        }
        self.last_attempt = Instant::now();
        self.sequence += 1;
        self.sender
            .as_ref()
            .unwrap()
            .send(Job::Write {
                session: self.session.clone(),
                sequence: self.sequence,
                source,
                project: Box::new(project),
                versions,
            })
            .map_err(|_| "Recovery writer stopped".to_string())?;
        self.busy = true;
        Ok(())
    }
    pub fn poll(&mut self) -> Vec<Completion> {
        let mut output = Vec::new();
        while let Ok((writing, result)) = self.results.try_recv() {
            if writing {
                self.busy = false;
            }
            output.push(Completion {
                written: result.as_ref().copied().unwrap_or(false),
                result: result.map(|_| ()),
            });
        }
        output
    }
    pub fn retire(&mut self) {
        let sender = self.sender.as_ref().unwrap();
        let _ = sender.send(Job::Retire(self.session.clone()));
        if let Some(session) = self.recovered_session.take() {
            let _ = sender.send(Job::Retire(session));
        }
        self.last_attempt = Instant::now();
    }
    pub fn recovered(&mut self, entry: &Entry) {
        // Keep the chosen session on disk until a manual save or deliberate discard,
        // even when restoring one of its versions repeatedly.
        let sender = self.sender.as_ref().unwrap();
        if self.session != entry.session {
            let _ = sender.send(Job::Retire(self.session.clone()));
        }
        if let Some(previous) = self.recovered_session.take()
            && previous != entry.session
        {
            let _ = sender.send(Job::Retire(previous));
        }
        self.recovered_session = (self.session != entry.session).then(|| entry.session.clone());
        self.last_attempt = Instant::now();
    }
}
impl Drop for Recovery {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn retire(directory: &Path, session: &str) -> Result<(), String> {
    for entry in entries(directory)?
        .into_iter()
        .filter(|e| e.session == session)
    {
        match std::fs::remove_file(entry.path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}
fn write(
    directory: &Path,
    session: &str,
    sequence: u32,
    source_path: String,
    project: Project,
    versions: usize,
    previous: &mut Option<(String, u64)>,
) -> Result<bool, String> {
    project.validate()?;
    let snapshot = Snapshot {
        version: 1,
        source_path,
        project,
    };
    let bytes = persistence::json_bytes(&snapshot)?;
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    let fingerprint = (session.to_owned(), hasher.finish());
    let changed = previous.as_ref() != Some(&fingerprint);
    if changed {
        std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        let path = directory.join(format!("recovery-{session}-{sequence:08x}.json"));
        persistence::write_bytes(&path, &bytes)?;
    }
    // Prune only after the new complete snapshot is committed, never before it.
    for old in entries(directory)?.into_iter().skip(versions.max(1)) {
        std::fs::remove_file(old.path).map_err(|e| e.to_string())?;
    }
    *previous = Some(fingerprint);
    Ok(changed)
}

#[cfg(test)]
pub(crate) mod tests;
