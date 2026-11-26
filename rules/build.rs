// Transpiles ox/reach.ox into Rust at build time (the dungeon_oxide pattern).
//
// If an `oxidized` binary is available (OXIDIZED env var, else PATH) it is the
// source of truth. If not, the committed copy in ox_gen/reach_gen.rs is used so
// the game still builds on machines (and in the web build) without Oxidized
// installed. After editing ox/reach.ox run `rules/regen.sh` to refresh the copy.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let src = manifest.join("ox/reach.ox");
    let committed = manifest.join("ox_gen/reach_gen.rs");
    println!("cargo:rerun-if-changed={}", src.display());
    println!("cargo:rerun-if-changed={}", committed.display());
    println!("cargo:rerun-if-env-changed=OXIDIZED");

    let exe = std::env::var("OXIDIZED").unwrap_or_else(|_| "oxidized".into());
    let raw = out_dir.join("reach_raw.rs");
    let ran = Command::new(&exe).arg("build").arg(&src).arg("-o").arg(&raw).output();
    let generated = match ran {
        Ok(out) if out.status.success() => {
            let code = std::fs::read_to_string(&raw).expect("read transpiled output");
            postprocess(&code)
        }
        Ok(out) => panic!("oxidized failed on {}:\n{}", src.display(), String::from_utf8_lossy(&out.stderr)),
        Err(_) => {
            println!("cargo:warning=`{exe}` not found; using committed ox_gen/reach_gen.rs");
            std::fs::read_to_string(&committed).expect("ox_gen/reach_gen.rs missing and no oxidized binary")
        }
    };
    std::fs::write(out_dir.join("reach_gen.rs"), &generated).unwrap();
    // Exposed so regen.sh can copy the exact bytes.
    println!("cargo:rustc-env=REACH_GEN_PATH={}", out_dir.join("reach_gen.rs").display());
}

/// Drops the program's `main` and makes the generated `fn`s public, so the
/// wrapper can call them. Runtime helpers (`__ox_*`) keep their spelling.
fn postprocess(code: &str) -> String {
    let (pre, main_part) = split_at_main(code);
    let mut out = String::with_capacity(code.len());
    for line in pre.lines() {
        match line.strip_prefix("fn ") {
            Some(rest) if rest.chars().next().is_some_and(|c| c.is_ascii_lowercase()) => {
                out.push_str("pub fn ");
                out.push_str(rest);
                out.push('\n');
            }
            _ => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    // The generated `main` (thread + stack-size launcher) is unused here.
    let _ = main_part;
    out
}

fn split_at_main(code: &str) -> (&str, &str) {
    let mut offset = 0usize;
    for line in code.lines() {
        if line.starts_with("fn main(") {
            // include the `#[allow]` attribute line that precedes it, if any
            return (&code[..offset], &code[offset..]);
        }
        offset += line.len() + 1;
    }
    (code, "")
}
