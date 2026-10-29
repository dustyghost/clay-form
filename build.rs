fn main() {
    // Track source edits and commits, including linked worktrees. Never watch target/.
    for path in [
        "build.rs",
        "src",
        "Cargo.toml",
        "Cargo.lock",
        "README.md",
        "docs",
        "legal",
        "LICENSE",
        "COPYRIGHT.md",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    for name in ["HEAD", "index", "refs", "packed-refs"] {
        if let Ok(output) = std::process::Command::new("git")
            .args(["rev-parse", "--git-path", name])
            .output()
            && output.status.success()
        {
            let path = String::from_utf8_lossy(&output.stdout);
            let path = path.trim();
            if std::path::Path::new(path).exists() {
                println!("cargo:rerun-if-changed={path}");
            }
        }
    }
    if std::path::Path::new("SOURCE-COMMIT").exists() {
        println!("cargo:rerun-if-changed=SOURCE-COMMIT");
    }
    // No paths or environment dump: only source identity and build mode.
    let revision = std::process::Command::new("git")
        .args(["describe", "--always", "--dirty", "--abbrev=12"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .or_else(|| {
            std::fs::read_to_string("SOURCE-COMMIT")
                .ok()
                .map(|s| s.trim().to_owned())
                .filter(|s| s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        })
        .unwrap_or_else(|| "source-revision-unavailable".into());
    println!(
        "cargo:rustc-env=CLAY_BUILD_ID={revision} ({})",
        std::env::var("PROFILE").unwrap_or_default()
    );
}
