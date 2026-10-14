use super::*;
use crate::{
    document::Document,
    sculpt::{Geometry, Primitive, PrimitiveKind},
};

pub(crate) struct Directory(pub PathBuf);
impl Directory {
    pub fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        Self(std::env::temp_dir().join(format!(
            "clay-recovery-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        )))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
pub(crate) fn document() -> Document {
    let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
    primitive.divisions = [2; 3];
    let mut geometry = Geometry::new(primitive.mesh().unwrap());
    geometry.mesh.mask[0] = 0.75;
    geometry
        .mesh
        .create_vertex_group("Protected".into(), 0.5, false)
        .unwrap();
    geometry.subdivide().unwrap();
    let mut document = Document::default();
    document.replace_geometry(geometry).unwrap();
    document.duplicate_model(false).unwrap();
    document
}
const SESSION: &str = "00000000000000000000000000000001-00000001";

#[test]
fn snapshots_round_trip_complete_projects_and_keep_only_changed_versions() {
    let directory = Directory::new();
    let original = document().project();
    let mut previous = None;
    assert!(
        write(
            &directory.0,
            SESSION,
            1,
            "never-saved.clay".into(),
            original.clone(),
            2,
            &mut previous
        )
        .unwrap()
    );
    let first = entries(&directory.0).unwrap().remove(0);
    let (loaded, path) = load(&first).unwrap();
    assert_eq!(path, "never-saved.clay");
    assert!(loaded == original);
    assert!(
        !write(
            &directory.0,
            SESSION,
            2,
            path.clone(),
            original.clone(),
            2,
            &mut previous
        )
        .unwrap()
    );
    assert_eq!(entries(&directory.0).unwrap().len(), 1);
    for i in 3..=5 {
        let mut changed = original.clone();
        changed.model_name = format!("Version {i}");
        write(
            &directory.0,
            SESSION,
            i,
            path.clone(),
            changed,
            2,
            &mut previous,
        )
        .unwrap();
    }
    let kept = entries(&directory.0).unwrap();
    assert_eq!(kept.len(), 2);
    assert_eq!(load(&kept[0]).unwrap().0.model_name, "Version 5");
    assert_eq!(load(&kept[1]).unwrap().0.model_name, "Version 4");
    assert!(!first.path.exists());
}
#[test]
fn interrupted_or_invalid_write_preserves_previous_copy_and_manual_project() {
    let directory = Directory::new();
    let project = document().project();
    let mut previous = None;
    write(
        &directory.0,
        SESSION,
        1,
        "original.clay".into(),
        project.clone(),
        1,
        &mut previous,
    )
    .unwrap();
    let original = directory.0.join("original.clay");
    persistence::save_project(&original.to_string_lossy(), &project).unwrap();
    let manual_bytes = std::fs::read(&original).unwrap();
    let abandoned = directory
        .0
        .join(format!("recovery-{SESSION}-00000002.json.tmp-123-0"));
    std::fs::write(abandoned, b"{incomplete").unwrap();
    let mut invalid = project.clone();
    invalid.version = 999;
    assert!(
        write(
            &directory.0,
            SESSION,
            2,
            "original.clay".into(),
            invalid,
            1,
            &mut previous
        )
        .is_err()
    );
    let candidates = entries(&directory.0).unwrap();
    assert_eq!(candidates.len(), 1);
    assert!(load(&candidates[0]).unwrap().0 == project);
    assert_eq!(std::fs::read(original).unwrap(), manual_bytes);
    // A failed atomic replacement cannot touch another recovery file.
    assert!(persistence::write_bytes(&directory.0, b"bad destination").is_err());
    assert!(load(&candidates[0]).is_ok());
}
#[test]
fn retiring_during_write_cannot_resurrect_saved_or_discarded_work() {
    let directory = Directory::new();
    let other = document().project();
    write(
        &directory.0,
        SESSION,
        1,
        "other.clay".into(),
        other.clone(),
        5,
        &mut None,
    )
    .unwrap();
    let mut worker = Recovery::new(directory.0.clone()).unwrap();
    worker.save(other, "new.clay".into(), 5).unwrap();
    worker.retire(); // Deliberately do not wait for the write.
    drop(worker); // Drain ordered jobs, as on a normal application exit.
    let kept = entries(&directory.0).unwrap();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].session, SESSION);
}
#[test]
fn failed_worker_write_reports_error_and_can_retry_without_blocking() {
    let directory = Directory::new();
    std::fs::create_dir_all(&directory.0).unwrap();
    let blocked = directory.0.join("blocked");
    std::fs::write(&blocked, b"file, not directory").unwrap();
    let mut worker = Recovery::new(blocked.clone()).unwrap();
    let project = document().project();
    worker.save(project.clone(), "source".into(), 2).unwrap();
    let wait = |worker: &mut Recovery| {
        let start = Instant::now();
        loop {
            let result = worker.poll();
            if !result.is_empty() {
                return result;
            }
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(2));
        }
    };
    assert!(wait(&mut worker)[0].result.is_err());
    assert!(!worker.busy);
    std::fs::remove_file(blocked).unwrap();
    worker.save(project, "source".into(), 2).unwrap();
    assert!(wait(&mut worker)[0].result.is_ok());
    assert_eq!(entries(&worker.directory).unwrap().len(), 1);
}
#[test]
fn interval_and_legacy_preferences_are_validated() {
    let directory = Directory::new();
    let mut worker = Recovery::new(directory.0.clone()).unwrap();
    let mut settings = Settings::default();
    assert!(!worker.due(&settings));
    worker.last_attempt -= Duration::from_secs(121);
    assert!(worker.due(&settings));
    settings.enabled = false;
    assert!(!worker.due(&settings));
    settings.interval_minutes = 0;
    assert!(settings.validate().is_err());
    let prefs: crate::preferences::Preferences = serde_json::from_str("{}").unwrap();
    assert_eq!(prefs.recovery, Settings::default());
}

#[test]
fn restoring_an_existing_copy_repeatedly_keeps_it_until_save_or_discard() {
    let directory = Directory::new();
    write(
        &directory.0,
        SESSION,
        1,
        "source.clay".into(),
        document().project(),
        5,
        &mut None,
    )
    .unwrap();
    let entry = entries(&directory.0).unwrap().remove(0);
    let mut worker = Recovery::new(directory.0.clone()).unwrap();
    worker.recovered(&entry);
    worker.recovered(&entry);
    drop(worker);
    assert!(entry.path.exists());
    let mut worker = Recovery::new(directory.0.clone()).unwrap();
    worker.recovered(&entry);
    worker.retire();
    drop(worker);
    assert!(!entry.path.exists());
}
