#!/usr/bin/env python3
"""Patch crates.io webrtc-audio-processing-sys 2.1.0 build.rs for MSVC (Windows CI)."""
from __future__ import annotations

import os
import pathlib
import subprocess
import sys


def cargo_home() -> pathlib.Path:
    raw = os.environ.get("CARGO_HOME")
    if raw:
        return pathlib.Path(raw)
    return pathlib.Path.home() / ".cargo"


def require_replace(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"APM {label} anchor not found")
    return text.replace(old, new, 1)


def main() -> None:
    subprocess.check_call(["cargo", "fetch"])
    registry = cargo_home() / "registry" / "src"
    if not registry.is_dir():
        raise SystemExit(f"cargo registry src missing after fetch: {registry}")

    builds = sorted(registry.glob("*/webrtc-audio-processing-sys-2.1.0/build.rs"))
    if not builds:
        raise SystemExit(
            f"webrtc-audio-processing-sys-2.1.0 build.rs not under {registry}"
        )

    path = None
    text = ""
    for candidate in builds:
        candidate_text = candidate.read_text(encoding="utf-8").replace("\r\n", "\n")
        if 'flag("-std=c++17")' in candidate_text or "flag_if_supported" in candidate_text:
            path = candidate
            text = candidate_text
            break
    if path is None:
        raise SystemExit(
            "no webrtc-audio-processing-sys-2.1.0 build.rs looked like upstream/patched APM"
        )

    # Already patched (e.g. warm cargo cache on a reused runner).
    if "skip prefix on MSVC" in text and "flag_if_supported" in text and "{name}.lib" in text:
        print(f"APM build.rs already patched: {path}")
        return

    orig = text

    text = require_replace(
        text,
        '        .flag("-std=c++17")\n        .flag("-Wno-unused-parameter")',
        '        .std("c++17")\n        .flag_if_supported("-Wno-unused-parameter")',
        "CC flags",
    )

    text = require_replace(
        text,
        "    let renamed_symbols = webrtc::prefix_library_symbols(&lib_dirs, SYMBOL_PREFIX)?;",
        """\
    // Nemo Windows CI: skip prefix on MSVC (nm/objcopy + lib*.a vs *.lib).
    let renamed_symbols = if cfg!(target_env = "msvc") {
        Vec::new()
    } else {
        webrtc::prefix_library_symbols(&lib_dirs, SYMBOL_PREFIX)?
    };""",
        "prefix_library_symbols",
    )

    text = require_replace(
        text,
        """\
    if cfg!(feature = "bundled") {
        println!("cargo:rustc-link-lib=static={LIB_NAME}");
        println!("cargo:rustc-link-lib=absl_strings");
    } else {
""",
        """\
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
""",
        "bundled link",
    )

    text = require_replace(
        text,
        """\
    let objcopy = sysroot.join("lib").join("rustlib").join(host).join("bin").join("rust-objcopy");

    // Optional: verification
    if !objcopy.exists() {
        println!("cargo:warning=rust-objcopy not found at {:?}", objcopy);
        println!("cargo:warning=Ensure the 'llvm-tools' component is installed: 'rustup component add llvm-tools'");
    }

    Ok(objcopy)
""",
        """\
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
""",
        "determine_objcopy_path",
    )

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
