use super::*;
use crate::{
    preferences,
    sculpt::{Mesh, Vec3},
};

fn triangle() -> Mesh {
    Mesh {
        vertices: vec![Vec3(0., 0., 0.), Vec3(1., 0., 0.), Vec3(0., 1., 0.)],
        mask: vec![0.; 3],
        triangles: vec![[0, 1, 2]],
        ..Default::default()
    }
}

#[test]
fn mesh_capacity_preferences_migrate_roundtrip_and_validate() {
    let mut prefs = preferences::Preferences::default();
    let mut legacy = serde_json::to_value(&prefs).unwrap();
    legacy.as_object_mut().unwrap().remove("mesh_capacity");
    let legacy: preferences::Preferences = serde_json::from_value(legacy).unwrap();
    assert_eq!(legacy.mesh_capacity, MeshCapacity::DEFAULT);
    prefs.mesh_capacity = MeshCapacity {
        vertices: 6_000_000,
        triangles: 12_000_000,
    };
    let path =
        std::env::temp_dir().join(format!("clay-capacity-prefs-{}.json", std::process::id()));
    preferences::save(&path, &prefs).unwrap();
    assert_eq!(
        preferences::load(&path).unwrap().mesh_capacity,
        prefs.mesh_capacity
    );
    std::fs::remove_file(path).unwrap();
    for limits in [
        MeshCapacity {
            vertices: 0,
            ..MeshCapacity::DEFAULT
        },
        MeshCapacity {
            triangles: 999_999,
            ..MeshCapacity::DEFAULT
        },
        MeshCapacity {
            vertices: usize::MAX,
            ..MeshCapacity::DEFAULT
        },
        MeshCapacity {
            triangles: MeshCapacity::MAX.triangles + 1,
            ..MeshCapacity::MAX
        },
    ] {
        prefs.mesh_capacity = limits;
        assert!(prefs.validate().is_err());
    }
    assert!(MeshCapacity::MAX.validate().is_ok());
}

#[test]
fn mesh_capacity_limits_cover_validation_subdivision_merge_and_import() {
    let mesh = triangle();
    for limits in [
        MeshCapacity {
            vertices: 5,
            triangles: 4,
        },
        MeshCapacity {
            vertices: 6,
            triangles: 3,
        },
    ] {
        with_test_capacity(limits, || assert!(mesh.subdivide().is_err()));
    }
    with_test_capacity(
        MeshCapacity {
            vertices: 6,
            triangles: 4,
        },
        || {
            let result = mesh.subdivide().unwrap();
            assert_eq!((result.vertices.len(), result.triangles.len()), (6, 4));
            result.validate().unwrap();
            assert_eq!(mesh.merged_with(&mesh).unwrap().vertices.len(), 6);
        },
    );
    with_test_capacity(
        MeshCapacity {
            vertices: 5,
            triangles: 4,
        },
        || {
            assert!(mesh.merged_with(&mesh).is_err());
        },
    );
    let path =
        std::env::temp_dir().join(format!("clay-capacity-import-{}.obj", std::process::id()));
    std::fs::write(&path, "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();
    with_test_capacity(
        MeshCapacity {
            vertices: 2,
            triangles: 4,
        },
        || {
            assert!(
                crate::persistence::load_obj(path.to_str().unwrap())
                    .unwrap_err()
                    .contains("vertex limit (2)")
            );
        },
    );
    with_test_capacity(
        MeshCapacity {
            vertices: 3,
            triangles: 1,
        },
        || {
            assert!(crate::persistence::load_obj(path.to_str().unwrap()).is_ok());
        },
    );
    std::fs::remove_file(path).unwrap();
    assert_eq!(current(), MeshCapacity::DEFAULT);
}

#[test]
fn mesh_capacity_accepts_counts_above_old_cap_and_preserves_project_io() {
    let mut mesh = Mesh::default();
    mesh.vertices
        .resize(MeshCapacity::DEFAULT.vertices + 1, Vec3::default());
    mesh.mask.resize(mesh.vertices.len(), 0.);
    assert!(mesh.validate().unwrap_err().contains("configured limits"));
    let limits = MeshCapacity {
        vertices: 1_000_000,
        triangles: 2_000_000,
    };
    let path =
        std::env::temp_dir().join(format!("clay-capacity-project-{}.clay", std::process::id()));
    with_test_capacity(limits, || {
        mesh.validate().unwrap();
        let mut project = crate::document::Document::default().project();
        project.mesh = mesh;
        project.mesh_settings.primitive = None;
        crate::persistence::save_project(path.to_str().unwrap(), &project).unwrap();
        let restored = crate::persistence::load_project(path.to_str().unwrap()).unwrap();
        assert_eq!(restored.mesh.vertices.len(), 500_001);
    });
    assert!(
        crate::persistence::load_project(path.to_str().unwrap())
            .err()
            .unwrap()
            .contains("configured limits")
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn mesh_capacity_startup_budget_is_shared_with_workers_and_immutable() {
    // A fresh process tests OnceLock initialization without changing other tests' budgets.
    const CHILD: &str = "CLAY_TEST_CAPACITY_STARTUP";
    let limits = MeshCapacity {
        vertices: 6_000_000,
        triangles: 12_000_000,
    };
    if std::env::var_os(CHILD).is_none() {
        let config =
            std::env::temp_dir().join(format!("clay-capacity-startup-{}", std::process::id()));
        let prefs = preferences::Preferences {
            mesh_capacity: limits,
            ..Default::default()
        };
        preferences::save(&config.join("clay-form/preferences.json"), &prefs).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["sculpt::capacity::tests::mesh_capacity_startup_budget_is_shared_with_workers_and_immutable", "--exact", "--nocapture"])
            .env(CHILD,"1").env("XDG_CONFIG_HOME", &config).output().unwrap();
        std::fs::remove_dir_all(config).unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        return;
    }
    let _app = crate::app::App::new();
    assert_eq!(current(), limits);
    let mut worker =
        crate::background_job::BackgroundJob::start("mesh-capacity-test", |_| Ok(current()))
            .unwrap();
    initialize(MeshCapacity::DEFAULT).unwrap(); // changing saved preferences cannot change an active job
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Some(result) = worker.poll() {
            assert_eq!(result.unwrap(), limits);
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(current(), limits);
}

#[test]
fn mesh_capacity_triangle_budget_can_exceed_the_old_limit() {
    let mut mesh = triangle();
    mesh.triangles
        .resize(MeshCapacity::DEFAULT.triangles + 1, [0, 1, 2]);
    assert!(mesh.validate().is_err());
    with_test_capacity(
        MeshCapacity {
            vertices: 500_000,
            triangles: 2_000_000,
        },
        || mesh.validate().unwrap(),
    );
}
