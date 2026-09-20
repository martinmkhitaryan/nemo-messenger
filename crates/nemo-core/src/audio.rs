//! Capture, AEC/AGC/NS, Opus encode/decode for 1:1 calls (ADR-0028).
//!
//! Platform shells push 16-bit PCM (`javax.sound` on desktop, `org.webrtc` on
//! Android). This module runs APM on desktop Unix and Opus on every target,
//! then writes samples onto webrtc-rs.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::error::{CoreError, Result};

pub const SAMPLE_RATE: u32 = 48_000;
pub const CHANNELS: usize = 2;
pub const FRAME_MONO: usize = 960; // 20 ms @ 48 kHz
const APM_CHUNK: usize = 480; // 10 ms @ 48 kHz
const CAPTURE_CAP: usize = SAMPLE_RATE as usize; // 1 s mono

struct OpusPair {
    enc: *mut libopus_sys::OpusEncoder,
    dec: *mut libopus_sys::OpusDecoder,
}

unsafe impl Send for OpusPair {}

impl OpusPair {
    fn new(cbr: bool) -> Result<Self> {
        let mut err = 0;
        let enc = unsafe {
            libopus_sys::opus_encoder_create(
                SAMPLE_RATE as i32,
                CHANNELS as i32,
                libopus_sys::OPUS_APPLICATION_VOIP as i32,
                &mut err,
            )
        };
        if enc.is_null() || err != libopus_sys::OPUS_OK as i32 {
            return Err(CoreError::Call(format!("opus encoder: {err}")));
        }
        let mut err = 0;
        let dec = unsafe {
            libopus_sys::opus_decoder_create(SAMPLE_RATE as i32, CHANNELS as i32, &mut err)
        };
        if dec.is_null() || err != libopus_sys::OPUS_OK as i32 {
            unsafe { libopus_sys::opus_encoder_destroy(enc) };
            return Err(CoreError::Call(format!("opus decoder: {err}")));
        }
        unsafe {
            libopus_sys::opus_encoder_ctl(
                enc,
                libopus_sys::OPUS_SET_BITRATE_REQUEST as i32,
                32_000,
            );
            libopus_sys::opus_encoder_ctl(enc, libopus_sys::OPUS_SET_INBAND_FEC_REQUEST as i32, 1);
            if cbr {
                libopus_sys::opus_encoder_ctl(enc, libopus_sys::OPUS_SET_VBR_REQUEST as i32, 0);
                libopus_sys::opus_encoder_ctl(enc, libopus_sys::OPUS_SET_DTX_REQUEST as i32, 0);
            }
        }
        Ok(Self { enc, dec })
    }

    fn encode_stereo(&mut self, interleaved: &[i16], out: &mut [u8]) -> Result<usize> {
        if interleaved.len() != FRAME_MONO * CHANNELS {
            return Err(CoreError::Call("opus frame size".into()));
        }
        let n = unsafe {
            libopus_sys::opus_encode(
                self.enc,
                interleaved.as_ptr(),
                FRAME_MONO as i32,
                out.as_mut_ptr(),
                out.len() as i32,
            )
        };
        if n < 0 {
            return Err(CoreError::Call(format!("opus encode {n}")));
        }
        Ok(n as usize)
    }

    fn decode_stereo(&mut self, packet: &[u8], pcm: &mut [i16]) -> Result<usize> {
        let n = unsafe {
            libopus_sys::opus_decode(
                self.dec,
                packet.as_ptr(),
                packet.len() as i32,
                pcm.as_mut_ptr(),
                FRAME_MONO as i32,
                0,
            )
        };
        if n < 0 {
            return Err(CoreError::Call(format!("opus decode {n}")));
        }
        Ok(n as usize)
    }
}

impl Drop for OpusPair {
    fn drop(&mut self) {
        unsafe {
            libopus_sys::opus_encoder_destroy(self.enc);
            libopus_sys::opus_decoder_destroy(self.dec);
        }
    }
}

pub struct AudioEngine {
    capture: Mutex<VecDeque<i16>>,
    playback: Mutex<VecDeque<i16>>,
    opus: Mutex<OpusPair>,
    #[cfg(all(not(target_os = "android"), unix))]
    apm: Option<webrtc_audio_processing::Processor>,
}

impl AudioEngine {
    pub fn new(cbr: bool) -> Result<Arc<Self>> {
        let opus = OpusPair::new(cbr)?;
        #[cfg(all(not(target_os = "android"), unix))]
        let apm = make_apm();
        Ok(Arc::new(Self {
            capture: Mutex::new(VecDeque::new()),
            playback: Mutex::new(VecDeque::new()),
            opus: Mutex::new(opus),
            #[cfg(all(not(target_os = "android"), unix))]
            apm,
        }))
    }

    #[cfg(test)]
    pub fn has_apm(&self) -> bool {
        #[cfg(all(not(target_os = "android"), unix))]
        {
            self.apm.is_some()
        }
        #[cfg(not(all(not(target_os = "android"), unix)))]
        {
            false
        }
    }

    pub fn stop(&self) {
        if let Ok(mut q) = self.capture.lock() {
            q.clear();
        }
        if let Ok(mut q) = self.playback.lock() {
            q.clear();
        }
    }

    /// Desktop/Android capture lives in the shell. Always false here.
    pub fn start_local_io(self: &Arc<Self>) -> bool {
        let _ = self;
        false
    }

    pub fn push_capture(&self, samples: &[i16], sample_rate: u32, channels: u32) {
        let mono = to_mono(samples, channels.max(1) as usize);
        let pcm = resample(&mono, sample_rate.max(1), SAMPLE_RATE);
        if let Ok(mut q) = self.capture.lock() {
            q.extend(pcm);
            while q.len() > CAPTURE_CAP {
                q.pop_front();
            }
        }
    }

    pub fn pull_playback(&self, max_samples: usize) -> Vec<i16> {
        let mut out = Vec::with_capacity(max_samples);
        if let Ok(mut q) = self.playback.lock() {
            for _ in 0..max_samples {
                match q.pop_front() {
                    Some(s) => out.push(s),
                    None => break,
                }
            }
        }
        out
    }

    pub fn take_capture_frame(&self) -> Vec<i16> {
        let mut frame = vec![0i16; FRAME_MONO];
        if let Ok(mut q) = self.capture.lock() {
            for slot in frame.iter_mut() {
                *slot = q.pop_front().unwrap_or(0);
            }
        }
        self.process_capture(&mut frame);
        frame
    }

    pub fn encode_frame(&self, mono: &[i16]) -> Result<Vec<u8>> {
        let stereo = upmix_stereo(mono);
        let mut buf = vec![0u8; 4000];
        let mut opus = self
            .opus
            .lock()
            .map_err(|_| CoreError::Call("opus lock".into()))?;
        let n = opus.encode_stereo(&stereo, &mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }

    pub fn decode_rtp(&self, payload: &[u8]) {
        if payload.is_empty() {
            return;
        }
        let mut stereo = vec![0i16; FRAME_MONO * CHANNELS];
        let n = {
            let mut opus = match self.opus.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            match opus.decode_stereo(payload, &mut stereo) {
                Ok(n) => n,
                Err(_) => return,
            }
        };
        let mut mono = downmix_stereo(&stereo[..n * CHANNELS]);
        if mono.len() > FRAME_MONO {
            mono.truncate(FRAME_MONO);
        }
        while mono.len() < FRAME_MONO {
            mono.push(0);
        }
        self.process_render(&mut mono);
        if let Ok(mut q) = self.playback.lock() {
            q.extend(mono);
            while q.len() > CAPTURE_CAP {
                q.pop_front();
            }
        }
    }

    fn process_capture(&self, mono: &mut [i16]) {
        #[cfg(all(not(target_os = "android"), unix))]
        if let Some(apm) = self.apm.as_ref() {
            for chunk in mono.chunks_mut(APM_CHUNK) {
                if chunk.len() != APM_CHUNK {
                    break;
                }
                let ch: Vec<f32> = chunk.iter().map(|&s| s as f32 / 32768.0).collect();
                let mut frame = vec![ch];
                if apm.process_capture_frame(&mut frame).is_ok() {
                    for (dst, src) in chunk.iter_mut().zip(frame[0].iter()) {
                        *dst = (src.clamp(-1.0, 1.0) * 32767.0) as i16;
                    }
                }
            }
        }
        #[cfg(not(all(not(target_os = "android"), unix)))]
        {
            let _ = mono;
        }
    }

    fn process_render(&self, mono: &mut [i16]) {
        #[cfg(all(not(target_os = "android"), unix))]
        if let Some(apm) = self.apm.as_ref() {
            for chunk in mono.chunks_mut(APM_CHUNK) {
                if chunk.len() != APM_CHUNK {
                    break;
                }
                let ch: Vec<f32> = chunk.iter().map(|&s| s as f32 / 32768.0).collect();
                let mut frame = vec![ch];
                let _ = apm.process_render_frame(&mut frame);
                for (dst, src) in chunk.iter_mut().zip(frame[0].iter()) {
                    *dst = (src.clamp(-1.0, 1.0) * 32767.0) as i16;
                }
            }
        }
        #[cfg(not(all(not(target_os = "android"), unix)))]
        {
            let _ = mono;
        }
    }
}

#[cfg(all(not(target_os = "android"), unix))]
fn make_apm() -> Option<webrtc_audio_processing::Processor> {
    use webrtc_audio_processing::config::{
        EchoCanceller, GainController, GainController1, GainControllerMode, HighPassFilter,
        NoiseSuppression,
    };
    use webrtc_audio_processing::{Config, Processor};

    let ap = Processor::new(SAMPLE_RATE).ok()?;
    ap.set_config(Config {
        echo_canceller: Some(EchoCanceller::default()),
        noise_suppression: Some(NoiseSuppression::default()),
        high_pass_filter: Some(HighPassFilter::default()),
        gain_controller: Some(GainController::GainController1(GainController1 {
            mode: GainControllerMode::AdaptiveDigital,
            analog_gain_controller: None,
            ..GainController1::default()
        })),
        ..Config::default()
    });
    Some(ap)
}

fn to_mono(samples: &[i16], channels: usize) -> Vec<i16> {
    if channels <= 1 {
        return samples.to_vec();
    }
    samples
        .chunks(channels)
        .map(|frame| {
            let sum: i32 = frame.iter().map(|s| *s as i32).sum();
            (sum / channels as i32) as i16
        })
        .collect()
}

fn upmix_stereo(mono: &[i16]) -> Vec<i16> {
    let mut out = Vec::with_capacity(mono.len() * 2);
    for &s in mono {
        out.push(s);
        out.push(s);
    }
    out
}

fn downmix_stereo(interleaved: &[i16]) -> Vec<i16> {
    interleaved
        .chunks(2)
        .map(|c| {
            if c.len() == 2 {
                ((c[0] as i32 + c[1] as i32) / 2) as i16
            } else {
                c[0]
            }
        })
        .collect()
}

fn resample(input: &[i16], from: u32, to: u32) -> Vec<i16> {
    if from == 0 || to == 0 || input.is_empty() {
        return Vec::new();
    }
    if from == to {
        return input.to_vec();
    }
    let out_len = (input.len() as u64 * to as u64 / from as u64).max(1) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 * from as f64 / to as f64;
        let i0 = (src.floor() as usize).min(input.len() - 1);
        let i1 = (i0 + 1).min(input.len() - 1);
        let frac = src - i0 as f64;
        let s = input[i0] as f64 * (1.0 - frac) + input[i1] as f64 * frac;
        out.push(s.round().clamp(i16::MIN as f64, i16::MAX as f64) as i16);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_roundtrip_keeps_tone() {
        let eng = AudioEngine::new(false).expect("opus");
        let mut pcm = vec![0i16; FRAME_MONO];
        for (i, s) in pcm.iter_mut().enumerate() {
            *s = ((i as f32 * 440.0 * 2.0 * std::f32::consts::PI / SAMPLE_RATE as f32).sin()
                * 12_000.0) as i16;
        }
        let encoded = eng.encode_frame(&pcm).expect("encode");
        assert!(encoded.len() > 3);
        assert_ne!(encoded, vec![0xF8, 0xFF, 0xFE]);
        eng.decode_rtp(&encoded);
        let decoded = eng.pull_playback(FRAME_MONO);
        let energy: i64 = decoded.iter().map(|s| i64::from(*s) * i64::from(*s)).sum();
        assert!(energy > 1_000_000, "energy={energy}");
    }

    #[cfg(all(unix, not(target_os = "android")))]
    #[test]
    fn apm_initializes_on_desktop() {
        let eng = AudioEngine::new(false).expect("opus");
        assert!(
            eng.has_apm(),
            "webrtc-audio-processing must init (ADR-0028)"
        );
    }

    fn tone() -> Vec<i16> {
        (0..FRAME_MONO)
            .map(|i| {
                ((i as f32 * 440.0 * 2.0 * std::f32::consts::PI / SAMPLE_RATE as f32).sin()
                    * 12_000.0) as i16
            })
            .collect()
    }

    #[test]
    fn private_opus_cbr_hides_silence_length() {
        let vbr = AudioEngine::new(false).expect("opus");
        let cbr = AudioEngine::new(true).expect("opus");
        let silence = vec![0i16; FRAME_MONO];
        let tone = tone();
        let mut vbr_silent = 0usize;
        let mut vbr_tone = 0usize;
        let mut cbr_silent = 0usize;
        let mut cbr_tone = 0usize;
        for _ in 0..8 {
            vbr_silent = vbr.encode_frame(&silence).expect("vbr silence").len();
            vbr_tone = vbr.encode_frame(&tone).expect("vbr tone").len();
            cbr_silent = cbr.encode_frame(&silence).expect("cbr silence").len();
            cbr_tone = cbr.encode_frame(&tone).expect("cbr tone").len();
        }
        assert!(
            vbr_tone > vbr_silent + 8,
            "VBR should shrink silence: silent={vbr_silent} tone={vbr_tone}"
        );
        let delta = (cbr_silent as i32 - cbr_tone as i32).unsigned_abs();
        assert!(
            delta <= 4,
            "CBR packet sizes must not leak speech: silent={cbr_silent} tone={cbr_tone}"
        );
    }
}
