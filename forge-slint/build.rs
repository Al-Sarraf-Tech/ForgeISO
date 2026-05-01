use std::process::Command;

fn main() {
    slint_build::compile("ui/app.slint").expect("Slint build failed");

    // Surface a short git hash to env so the StatusBar shows the build identity.
    // Falls back to empty string when not built from a git checkout (release tarball).
    let hash = Command::new("git")
        .args(["rev-parse", "--short=8", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    println!("cargo:rustc-env=FORGEISO_BUILD_HASH={}", hash);
    println!("cargo:rerun-if-changed=.git/HEAD");
}
