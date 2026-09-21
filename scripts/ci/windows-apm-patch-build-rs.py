#!/usr/bin/env python3
"""Patch crates.io webrtc-audio-processing-sys 2.1.0 build.rs for MSVC (Windows CI)."""
from __future__ import annotations

import pathlib
import subprocess
import sys


def main() -> None:
    subprocess.check_call(["cargo", "fetch"])
    registry = pathlib.Path.home() / ".cargo" / "registry" / "src"
    if not registry.is_dir():
        raise SystemExit(f"cargo registry src missing after fetch: {registry}")

    builds = list(registry.glob("*/webrtc-audio-processing-sys-2.1.0/build.rs"))
    if not builds:
        raise SystemExit("webrtc-audio-processing-sys-2.1.0 build.rs not fetched")
    path = builds[0]
    text = path.read_text(encoding="utf-8").replace("\r\n", "\n")
    orig = text

    text = text.replace(
        '        .flag("-std=c++17")\n        .flag("-Wno-unused-parameter")',
        '        .std("c++17")\n        .flag_if_supported("-Wno-unused-parameter")',
    )

    old_prefix = (
        "    let renamed_symbols = webrtc::prefix_library_symbols(&lib_dirs, SYMBOL_PREFIX)?;"
    )
    new_prefix = """\
    // Nemo Windows CI: skip prefix on MSVC (nm/objcopy + lib*.a vs *.lib).
    let renamed_symbols = if cfg!(target_env = "msvc") {
        Vec::new()
    } else {
        webrtc::prefix_library_symbols(&lib_dirs, SYMBOL_PREFIX)?
    };"""
    if old_prefix not in text:
        raise SystemExit("APM prefix_library_symbols anchor not found")
    text = text.replace(old_prefix, new_prefix, 1)

    old_link = """\
    if cfg!(feature = "bundled") {
        println!("cargo:rustc-link-lib=static={LIB_NAME}");
        println!("cargo:rustc-link-lib=absl_strings");
    } else {
"""
    new_link = """\
    if cfg!(feature = "bundled") {
        if cfg!(target_env = "msvc") {
            for dir in &lib_dirs {
                if let Ok(rd) = std::fs::read_dir(dir) {
                    for ent in rd.flatten() {
                        let p = ent.path();
                        if p.extension().and_then(|e| e.to_str()) != Some("a") {
                            continue;
                        }
                        let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else { continue };
                        let name = stem.strip_prefix("lib").unwrap_or(stem);
                        let lib = p.with_file_name(format!("{name}.lib"));
                        if !lib.exists() {
                            let _ = std::fs::copy(&p, &lib);
                        }
                    }
                }
            }
        }
        println!("cargo:rustc-link-lib=static={LIB_NAME}");
        println!("cargo:rustc-link-lib=absl_strings");
        if cfg!(target_env = "msvc") {
            for dir in &lib_dirs {
                if let Ok(rd) = std::fs::read_dir(dir) {
                    for ent in rd.flatten() {
                        let p = ent.path();
                        if p.extension().and_then(|e| e.to_str()) != Some("lib") {
                            continue;
                        }
                        let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else { continue };
                        if stem.starts_with("absl_") && stem != "absl_strings" {
                            println!("cargo:rustc-link-lib={stem}");
                        }
                    }
                }
            }
        }
    } else {
"""
    if old_link not in text:
        raise SystemExit("APM bundled link anchor not found")
    text = text.replace(old_link, new_link, 1)

    old_obj = """\
    let objcopy = sysroot.join("lib").join("rustlib").join(host).join("bin").join("rust-objcopy");

    // Optional: verification
    if !objcopy.exists() {
        println!("cargo:warning=rust-objcopy not found at {:?}", objcopy);
        println!("cargo:warning=Ensure the 'llvm-tools' component is installed: 'rustup component add llvm-tools'");
    }

    Ok(objcopy)
"""
    new_obj = """\
    let bin = sysroot.join("lib").join("rustlib").join(host).join("bin");
    let candidates = [
        bin.join("rust-objcopy.exe"),
        bin.join("rust-objcopy"),
        bin.join("llvm-objcopy.exe"),
        bin.join("llvm-objcopy"),
    ];
    let objcopy = candidates.into_iter().find(|p| p.exists()).unwrap_or_else(|| bin.join("rust-objcopy"));
    if !objcopy.exists() {
        println!("cargo:warning=rust-objcopy not found under {:?}", bin);
        println!("cargo:warning=Ensure the 'llvm-tools' component is installed: 'rustup component add llvm-tools'");
    }
    Ok(objcopy)
"""
    if old_obj not in text:
        raise SystemExit("APM determine_objcopy_path anchor not found")
    text = text.replace(old_obj, new_obj, 1)

    if text == orig:
        raise SystemExit("APM build.rs patch produced no changes")
    if "flag_if_supported" not in text:
        raise SystemExit("APM CC flag patch missing")
    if "skip prefix on MSVC" not in text:
        raise SystemExit("APM MSVC prefix skip patch missing")
    if "{name}.lib" not in text:
        raise SystemExit("APM .lib mirror patch missing")

    path.write_text(text, encoding="utf-8", newline="\n")
    print(f"Patched MSVC APM build.rs: {path}")


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as e:
        sys.exit(e.returncode)
