use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/sfx/sfx_stub.c");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let target_stub = out_dir.join("sfx_elf_stub.bin");
    let c_source = PathBuf::from("assets/sfx/sfx_stub.c");
    let prebuilt_stub = PathBuf::from("assets/sfx/sfx_elf_stub_x86_64.bin");

    // Attempt to compile with gcc if available
    let mut compiled_ok = false;
    if c_source.exists() {
        let status = Command::new("gcc")
            .args([
                "-Wall",
                "-Wextra",
                "-Os",
                "-s",
                c_source.to_str().unwrap(),
                "-o",
                target_stub.to_str().unwrap(),
            ])
            .status();

        if let Ok(st) = status
            && st.success()
        {
            compiled_ok = true;
        }
    }

    // Fall back to prebuilt stub if gcc compilation wasn't successful
    if !compiled_ok {
        if prebuilt_stub.exists() {
            fs::copy(&prebuilt_stub, &target_stub)
                .expect("Failed to copy prebuilt SFX stub to OUT_DIR");
        } else {
            panic!("Neither gcc compilation nor prebuilt SFX stub is available.");
        }
    }
}
