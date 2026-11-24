//! Builds galdr's Calx core (`core/galdr.calx`) as a static library and links it.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let core_dir = manifest.join("../../core").canonicalize().expect("core/ next to prototypes/");
    let main_file = core_dir.join("galdr.calx");
    println!("cargo:rerun-if-changed={}", core_dir.display());
    println!("cargo:rerun-if-env-changed=CALX");

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let lib = out.join(if cfg!(windows) { "galdr_core.lib" } else { "libgaldr_core.a" });
    let calx = std::env::var("CALX").unwrap_or_else(|_| "calx".into());
    let mut cmd = Command::new(&calx);
    cmd.args(["build", "--staticlib"]);
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        cmd.arg("--release");
    }
    cmd.arg(&main_file).arg("-o").arg(&lib);
    let result = cmd.output().unwrap_or_else(|e| panic!("running {calx} (is it on PATH?): {e}"));
    if !result.status.success() {
        panic!(
            "calx build failed:\n{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }

    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=galdr_core");
    if cfg!(target_os = "linux") {
        for l in ["m", "pthread", "dl"] {
            println!("cargo:rustc-link-lib=dylib={l}");
        }
    }
}
