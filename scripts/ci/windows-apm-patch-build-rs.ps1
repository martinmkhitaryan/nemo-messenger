# Patch crates.io webrtc-audio-processing-sys 2.1.0 build.rs for MSVC (Windows CI).
# Upstream is Unix-oriented: GCC flags, nm/objcopy symbol prefixing, lib*.a names.
$ErrorActionPreference = "Stop"

cargo fetch
$registrySrc = Join-Path $env:USERPROFILE ".cargo\registry\src"
if (-not (Test-Path $registrySrc)) { throw "cargo registry src missing after fetch" }

$apmBuild = Get-ChildItem -Path $registrySrc -Recurse -Filter "build.rs" |
  Where-Object { $_.Directory.Name -eq "webrtc-audio-processing-sys-2.1.0" } |
  Select-Object -First 1 -ExpandProperty FullName
if (-not $apmBuild) { throw "webrtc-audio-processing-sys-2.1.0 build.rs not fetched" }

$t = [System.IO.File]::ReadAllText($apmBuild)
$orig = $t

# 1) MSVC rejects GCC -Wno-*; use cc's portable helpers.
$t = $t.Replace(
  "        .flag(`"-std=c++17`")`n        .flag(`"-Wno-unused-parameter`")",
  "        .std(`"c++17`")`n        .flag_if_supported(`"-Wno-unused-parameter`")"
)

# 2) Skip nm/objcopy symbol prefixing on MSVC (tool/.a vs .lib mismatch; one APM in nemo-ffi).
$oldPrefix = @'
    let renamed_symbols = webrtc::prefix_library_symbols(&lib_dirs, SYMBOL_PREFIX)?;
'@
$newPrefix = @'
    // Nemo Windows CI: skip prefix on MSVC (nm/objcopy + lib*.a vs *.lib).
    let renamed_symbols = if cfg!(target_env = "msvc") {
        Vec::new()
    } else {
        webrtc::prefix_library_symbols(&lib_dirs, SYMBOL_PREFIX)?
    };
'@
if (-not $t.Contains($oldPrefix)) { throw "APM prefix_library_symbols anchor not found" }
$t = $t.Replace($oldPrefix, $newPrefix)

# 3) MSVC link.exe wants *.lib; Meson often installs lib*.a — mirror them.
$oldLink = @'
    if cfg!(feature = "bundled") {
        println!("cargo:rustc-link-lib=static={LIB_NAME}");
        println!("cargo:rustc-link-lib=absl_strings");
    } else {
'@
$newLink = @'
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
'@
if (-not $t.Contains($oldLink)) { throw "APM bundled link anchor not found" }
$t = $t.Replace($oldLink, $newLink)

# 4) rust-objcopy path: prefer .exe / llvm-objcopy on Windows.
$oldObj = @'
    let objcopy = sysroot.join("lib").join("rustlib").join(host).join("bin").join("rust-objcopy");

    // Optional: verification
    if !objcopy.exists() {
        println!("cargo:warning=rust-objcopy not found at {:?}", objcopy);
        println!("cargo:warning=Ensure the 'llvm-tools' component is installed: 'rustup component add llvm-tools'");
    }

    Ok(objcopy)
'@
$newObj = @'
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
'@
if (-not $t.Contains($oldObj)) { throw "APM determine_objcopy_path anchor not found" }
$t = $t.Replace($oldObj, $newObj)

if ($t -eq $orig) { throw "APM build.rs patch produced no changes" }
if (-not $t.Contains("flag_if_supported")) { throw "APM CC flag patch missing" }
if (-not $t.Contains("skip prefix on MSVC")) { throw "APM MSVC prefix skip patch missing" }
if (-not $t.Contains('{name}.lib')) { throw "APM .lib mirror patch missing" }

[System.IO.File]::WriteAllText($apmBuild, $t)
Write-Host "Patched MSVC APM build.rs: $apmBuild"
