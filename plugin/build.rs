use std::process::Command;

fn main() {
    let sha = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let sha = sha.trim();

    let pkg_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let version = if sha.is_empty() {
        pkg_version
    } else {
        format!("{pkg_version}+{sha}")
    };

    println!("cargo:rustc-env=ZELLIGENT_VERSION={version}");
    println!("cargo:rerun-if-changed=../.git/index");
}
