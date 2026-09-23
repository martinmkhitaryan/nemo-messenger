//! Native link flags for platform deps of the audio stack.
//!
//! Bundled `webrtc-audio-processing` on Windows references `timeGetTime`
//! (`winmm`); that import is not declared by the crate's build script.

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        println!("cargo:rustc-link-lib=winmm");
    }
}
