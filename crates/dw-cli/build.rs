use std::path::Path;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set");
    let version_path = Path::new(&manifest_dir).join("../../VERSION");
    println!("cargo:rerun-if-changed={}", version_path.display());
    println!("cargo:rerun-if-env-changed=DW_COMMIT");

    let version = std::fs::read_to_string(&version_path)
        .expect("VERSION should be readable")
        .trim()
        .to_owned();
    println!("cargo:rustc-env=DW_VERSION={version}");
    if let Ok(commit) = std::env::var("DW_COMMIT")
        && !commit.trim().is_empty()
    {
        println!("cargo:rustc-env=DW_COMMIT={commit}");
    }
}
