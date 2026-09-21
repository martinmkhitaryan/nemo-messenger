//! 1:1 voice: webrtc-rs ICE/DTLS-SRTP with `iceTransportPolicy=relay` (ADR-0024).
//! Signaling and DTLS fingerprint binding stay in this crate; TURN is coturn.

use std::env;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use rand::RngCore;
use rtc::interceptor::Registry;
use rtc::media::Sample;
use rtc::rtp_transceiver::rtp_sender::{
    RTCRtpCodec, RTCRtpCodingParameters, RTCRtpEncodingParameters, RtpCodecKind,
};
use tokio::time::{sleep, Instant};
use webrtc::media_stream::track_local::static_sample::TrackLocalStaticSample;
use webrtc::media_stream::track_local::TrackLocal;
use webrtc::media_stream::track_remote::{TrackRemote, TrackRemoteEvent};
use webrtc::media_stream::MediaStreamTrack;
use webrtc::peer_connection::{
    register_default_interceptors, MediaEngine, PeerConnection, PeerConnectionBuilder,
    PeerConnectionEventHandler, RTCConfigurationBuilder, RTCIceCandidateType,
    RTCIceConnectionState, RTCIceGatheringState, RTCIceServer, RTCIceTransportPolicy,
    RTCPeerConnectionIceEvent, RTCPeerConnectionState, RTCSessionDescription,
};

use crate::app::{random_call_id, reject_direct_ice, CALL_ID_LEN};
use crate::audio::AudioEngine;
use crate::error::{CoreError, Result};

const OPUS_PT: u8 = 111;
const MIME_OPUS: &str = "audio/opus";
/// 20 ms Opus DTX/silence TOC used so tests do not need libopus.
const OPUS_SILENCE: &[u8] = &[0xF8, 0xFF, 0xFE];

#[derive(Clone, Debug)]
pub struct TurnConfig {
    pub url: String,
    pub username: String,
    pub credential: String,
    pub bind: String,
}

impl TurnConfig {
    pub fn ice_bind() -> String {
        env::var("NEMO_ICE_BIND").unwrap_or_else(|_| {
            if cfg!(target_os = "android") {
                "0.0.0.0:0".into()
            } else {
                "127.0.0.1:0".into()
            }
        })
    }

    pub fn from_env() -> Self {
        Self {
            url: env::var("NEMO_TURN_URL").unwrap_or_else(|_| "turn:127.0.0.1:3478".into()),
            username: env::var("NEMO_TURN_USER").unwrap_or_else(|_| "nemo".into()),
            credential: env::var("NEMO_TURN_PASS").unwrap_or_else(|_| "nemo".into()),
            bind: Self::ice_bind(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LocalSignal {
    pub call_id: [u8; CALL_ID_LEN],
    pub sdp: String,
    pub dtls_fp: String,
    pub ice: Vec<String>,
}

struct CallEvents {
    ice: std::sync::Mutex<Vec<String>>,
    forbidden: AtomicBool,
    gathering_done: AtomicBool,
    connected_done: AtomicBool,
    received: AtomicU64,
    closed: AtomicBool,
    audio: Arc<AudioEngine>,
}

#[derive(Clone)]
struct Handler {
    events: Arc<CallEvents>,
}

#[async_trait::async_trait]
impl PeerConnectionEventHandler for Handler {
    async fn on_ice_candidate(&self, event: RTCPeerConnectionIceEvent) {
        match event.candidate.typ {
            RTCIceCandidateType::Host | RTCIceCandidateType::Srflx => {
                self.events.forbidden.store(true, Ordering::SeqCst);
            }
            RTCIceCandidateType::Relay => {
                if let Ok(init) = event.candidate.to_json() {
                    if let Ok(mut ice) = self.events.ice.lock() {
                        ice.push(init.candidate);
                    }
                }
            }
            _ => {}
        }
    }

    async fn on_ice_gathering_state_change(&self, state: RTCIceGatheringState) {
        if state == RTCIceGatheringState::Complete {
            self.events.gathering_done.store(true, Ordering::SeqCst);
        }
    }

    async fn on_ice_connection_state_change(&self, state: RTCIceConnectionState) {
        if state == RTCIceConnectionState::Connected || state == RTCIceConnectionState::Completed {
            self.events.connected_done.store(true, Ordering::SeqCst);
        }
    }

    async fn on_connection_state_change(&self, state: RTCPeerConnectionState) {
        if state == RTCPeerConnectionState::Connected {
            self.events.connected_done.store(true, Ordering::SeqCst);
        }
        if matches!(
            state,
            RTCPeerConnectionState::Failed
                | RTCPeerConnectionState::Closed
                | RTCPeerConnectionState::Disconnected
        ) {
            self.events.closed.store(true, Ordering::SeqCst);
        }
    }

    async fn on_track(&self, track: Arc<dyn TrackRemote>) {
        let events = Arc::clone(&self.events);
        tokio::spawn(async move {
            while let Some(ev) = track.poll().await {
                match ev {
                    TrackRemoteEvent::OnRtpPacket(pkt) => {
                        events.received.fetch_add(1, Ordering::Relaxed);
                        events.audio.decode_rtp(&pkt.payload);
                    }
                    TrackRemoteEvent::OnEnded => break,
                    _ => {}
                }
            }
        });
    }
}

/// One 1:1 media session. ICE is relay-only; host/srflx candidates are refused.
pub struct Call {
    pc: Arc<dyn PeerConnection>,
    events: Arc<CallEvents>,
    track: Arc<TrackLocalStaticSample>,
    ssrc: u32,
    call_id: [u8; CALL_ID_LEN],
    local: LocalSignal,
    audio: Arc<AudioEngine>,
    ice_sent: AtomicUsize,
}

impl Call {
    pub async fn offer(turn: &TurnConfig) -> Result<Self> {
        Self::offer_with(turn, false).await
    }

    pub async fn offer_with(turn: &TurnConfig, cbr: bool) -> Result<Self> {
        let call_id = random_call_id();
        Self::create(turn, call_id, None, cbr).await
    }

    pub async fn answer(turn: &TurnConfig, remote: &LocalSignal) -> Result<Self> {
        Self::answer_with(turn, remote, false).await
    }

    pub async fn answer_with(turn: &TurnConfig, remote: &LocalSignal, cbr: bool) -> Result<Self> {
        Self::create(turn, remote.call_id, Some(remote), cbr).await
    }

    async fn create(
        turn: &TurnConfig,
        call_id: [u8; CALL_ID_LEN],
        remote: Option<&LocalSignal>,
        cbr: bool,
    ) -> Result<Self> {
        let mut media_engine = MediaEngine::default();
        media_engine.register_default_codecs().map_err(call_err)?;
        let registry =
            register_default_interceptors(Registry::new(), &mut media_engine).map_err(call_err)?;
        let config = RTCConfigurationBuilder::new()
            .with_ice_servers(vec![RTCIceServer {
                urls: vec![turn.url.clone()],
                username: turn.username.clone(),
                credential: turn.credential.clone(),
                ..Default::default()
            }])
            .with_ice_transport_policy(RTCIceTransportPolicy::Relay)
            .build();
        let audio = AudioEngine::new(cbr)?;
        let events = Arc::new(CallEvents {
            ice: std::sync::Mutex::new(Vec::new()),
            forbidden: AtomicBool::new(false),
            gathering_done: AtomicBool::new(false),
            connected_done: AtomicBool::new(false),
            received: AtomicU64::new(0),
            closed: AtomicBool::new(false),
            audio: Arc::clone(&audio),
        });
        let handler = Arc::new(Handler {
            events: Arc::clone(&events),
        });
        let pc = PeerConnectionBuilder::new()
            .with_configuration(config)
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .with_handler(handler)
            .with_udp_addrs(vec![turn.bind.clone()])
            .build()
            .await
            .map_err(call_err)?;
        let pc: Arc<dyn PeerConnection> = Arc::new(pc);

        let mut ssrc_bytes = [0u8; 4];
        rand::rngs::OsRng.fill_bytes(&mut ssrc_bytes);
        let ssrc = u32::from_be_bytes(ssrc_bytes);
        let codec = RTCRtpCodec {
            mime_type: MIME_OPUS.into(),
            clock_rate: 48_000,
            channels: 2,
            sdp_fmtp_line: "minptime=10;useinbandfec=1".into(),
            rtcp_feedback: vec![],
        };
        let track = Arc::new(
            TrackLocalStaticSample::new(MediaStreamTrack::new(
                "nemo-audio".into(),
                "nemo-opus".into(),
                "nemo".into(),
                RtpCodecKind::Audio,
                vec![RTCRtpEncodingParameters {
                    rtp_coding_parameters: RTCRtpCodingParameters {
                        ssrc: Some(ssrc),
                        ..Default::default()
                    },
                    codec,
                    ..Default::default()
                }],
            ))
            .map_err(call_err)?,
        );
        pc.add_track(Arc::clone(&track) as Arc<dyn TrackLocal>)
            .await
            .map_err(call_err)?;

        if let Some(remote) = remote {
            verify_fingerprint(&remote.sdp, &remote.dtls_fp)?;
            pc.set_remote_description(
                RTCSessionDescription::offer(remote.sdp.clone()).map_err(call_err)?,
            )
            .await
            .map_err(call_err)?;
            add_ice(&*pc, &remote.ice).await?;
            let answer = pc.create_answer(None).await.map_err(call_err)?;
            pc.set_local_description(answer).await.map_err(call_err)?;
        } else {
            let offer = pc.create_offer(None).await.map_err(call_err)?;
            pc.set_local_description(offer).await.map_err(call_err)?;
        }

        wait_first_relay(&events, Duration::from_secs(15)).await?;
        if events.forbidden.load(Ordering::SeqCst) {
            let _ = pc.close().await;
            return Err(CoreError::DirectIceForbidden);
        }
        let ice = events
            .ice
            .lock()
            .map_err(|_| CoreError::Call("ice lock".into()))?
            .clone();
        reject_direct_ice(&ice)?;
        if ice.is_empty() {
            let _ = pc.close().await;
            return Err(CoreError::Call("no relay ICE candidates".into()));
        }
        let sdp = pc
            .local_description()
            .await
            .ok_or_else(|| CoreError::Call("missing local SDP".into()))?
            .sdp;
        let dtls_fp = fingerprint_from_sdp(&sdp)?;
        let ice_len = ice.len();
        Ok(Self {
            pc,
            events,
            track,
            ssrc,
            call_id,
            local: LocalSignal {
                call_id,
                sdp,
                dtls_fp,
                ice,
            },
            audio,
            ice_sent: AtomicUsize::new(ice_len),
        })
    }

    pub fn local(&self) -> &LocalSignal {
        &self.local
    }

    pub fn call_id(&self) -> [u8; CALL_ID_LEN] {
        self.call_id
    }

    pub async fn apply_answer(&self, remote: &LocalSignal) -> Result<()> {
        if remote.call_id != self.call_id {
            return Err(CoreError::Call("call_id mismatch".into()));
        }
        verify_fingerprint(&remote.sdp, &remote.dtls_fp)?;
        self.pc
            .set_remote_description(
                RTCSessionDescription::answer(remote.sdp.clone()).map_err(call_err)?,
            )
            .await
            .map_err(call_err)?;
        add_ice(&*self.pc, &remote.ice).await
    }

    pub async fn add_remote_ice(&self, ice: &[String]) -> Result<()> {
        add_ice(&*self.pc, ice).await
    }

    /// Relay candidates gathered after the invite/answer snapshot (`call_ice`).
    pub fn take_unsent_ice(&self) -> Result<Vec<String>> {
        if self.events.forbidden.load(Ordering::SeqCst) {
            return Err(CoreError::DirectIceForbidden);
        }
        let ice = self
            .events
            .ice
            .lock()
            .map_err(|_| CoreError::Call("ice lock".into()))?;
        let sent = self.ice_sent.load(Ordering::SeqCst);
        if ice.len() <= sent {
            return Ok(Vec::new());
        }
        let extra = ice[sent..].to_vec();
        self.ice_sent.store(ice.len(), Ordering::SeqCst);
        drop(ice);
        reject_direct_ice(&extra)?;
        Ok(extra)
    }

    pub fn ice_gathering_done(&self) -> bool {
        self.events.gathering_done.load(Ordering::SeqCst)
    }

    pub fn is_closed(&self) -> bool {
        self.events.closed.load(Ordering::SeqCst)
    }

    pub async fn wait_connected(&self) -> Result<()> {
        if self.events.closed.load(Ordering::SeqCst) {
            return Err(CoreError::Call("closed".into()));
        }
        wait_flag(
            &self.events.connected_done,
            Duration::from_secs(20),
            "ICE connected",
        )
        .await
    }

    pub fn received_rtp(&self) -> u64 {
        self.events.received.load(Ordering::Relaxed)
    }

    pub async fn send_silence_frames(&self, n: usize) -> Result<()> {
        let sample = Sample {
            data: Bytes::from_static(OPUS_SILENCE),
            duration: Duration::from_millis(20),
            ..Default::default()
        };
        for _ in 0..n {
            if self.events.closed.load(Ordering::SeqCst) {
                break;
            }
            self.track
                .write_sample(self.ssrc, OPUS_PT, &sample, &[])
                .await
                .map_err(call_err)?;
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        Ok(())
    }

    /// Start desktop cpal capture/playback when a device exists.
    pub fn start_audio_io(&self) -> bool {
        self.audio.start_local_io()
    }

    pub fn push_capture_pcm(&self, samples: &[i16], sample_rate: u32, channels: u32) {
        self.audio.push_capture(samples, sample_rate, channels);
    }

    pub fn pull_playback_pcm(&self, max_samples: u32) -> Vec<i16> {
        self.audio.pull_playback(max_samples as usize)
    }

    /// Encode mic PCM (or zeros) to Opus and write RTP. Used when a capture path is live.
    pub async fn send_capture_frames(&self, n: usize) -> Result<()> {
        for _ in 0..n {
            if self.events.closed.load(Ordering::SeqCst) {
                break;
            }
            let pcm = self.audio.take_capture_frame();
            let payload = match self.audio.encode_frame(&pcm) {
                Ok(bytes) if !bytes.is_empty() => Bytes::from(bytes),
                _ => Bytes::from_static(OPUS_SILENCE),
            };
            let sample = Sample {
                data: payload,
                duration: Duration::from_millis(20),
                ..Default::default()
            };
            self.track
                .write_sample(self.ssrc, OPUS_PT, &sample, &[])
                .await
                .map_err(call_err)?;
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        Ok(())
    }

    pub async fn close(&self) -> Result<()> {
        self.events.closed.store(true, Ordering::SeqCst);
        self.audio.stop();
        self.pc.close().await.map_err(call_err)
    }
}

async fn add_ice(pc: &dyn PeerConnection, ice: &[String]) -> Result<()> {
    reject_direct_ice(ice)?;
    for candidate in ice {
        pc.add_ice_candidate(webrtc::peer_connection::RTCIceCandidateInit {
            candidate: candidate.clone(),
            ..Default::default()
        })
        .await
        .map_err(call_err)?;
    }
    Ok(())
}

async fn wait_first_relay(events: &CallEvents, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if events.forbidden.load(Ordering::SeqCst) {
            return Err(CoreError::DirectIceForbidden);
        }
        let n = events.ice.lock().map(|g| g.len()).unwrap_or(0);
        if n > 0 {
            return Ok(());
        }
        if events.gathering_done.load(Ordering::SeqCst) {
            return Err(CoreError::Call("no relay ICE candidates".into()));
        }
        if Instant::now() >= deadline {
            return Err(CoreError::Call("ICE gathering timed out".into()));
        }
        sleep(Duration::from_millis(50)).await;
    }
}

async fn wait_flag(flag: &AtomicBool, timeout: Duration, what: &str) -> Result<()> {
    let deadline = Instant::now() + timeout;
    while !flag.load(Ordering::SeqCst) {
        if Instant::now() >= deadline {
            return Err(CoreError::Call(format!("{what} timed out")));
        }
        sleep(Duration::from_millis(50)).await;
    }
    Ok(())
}

fn call_err<E: std::fmt::Display>(err: E) -> CoreError {
    CoreError::Call(err.to_string())
}

pub fn fingerprint_from_sdp(sdp: &str) -> Result<String> {
    for line in sdp.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("a=fingerprint:") {
            let fp = rest.trim();
            if !fp.is_empty() {
                return Ok(fp.to_string());
            }
        }
    }
    Err(CoreError::Call("SDP has no a=fingerprint".into()))
}

pub fn fingerprints_match(a: &str, b: &str) -> bool {
    normalize_fp(a) == normalize_fp(b)
}

fn normalize_fp(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_hexdigit())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn verify_fingerprint(sdp: &str, claimed: &str) -> Result<()> {
    let got = fingerprint_from_sdp(sdp)?;
    if !fingerprints_match(&got, claimed) {
        return Err(CoreError::DtlsFingerprintMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{encode, AppBody, AppHeader, AppMessage};

    fn test_events() -> CallEvents {
        CallEvents {
            ice: std::sync::Mutex::new(Vec::new()),
            forbidden: AtomicBool::new(false),
            gathering_done: AtomicBool::new(false),
            connected_done: AtomicBool::new(false),
            received: AtomicU64::new(0),
            closed: AtomicBool::new(false),
            audio: AudioEngine::new(false).expect("opus"),
        }
    }

    #[tokio::test]
    async fn wait_first_relay_returns_when_a_candidate_arrives() {
        let events = test_events();
        events
            .ice
            .lock()
            .expect("ice")
            .push("candidate:1 1 udp 1 192.0.2.1 9 typ relay raddr 0.0.0.0 rport 0".into());
        wait_first_relay(&events, Duration::from_secs(1))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn wait_first_relay_fails_when_gathering_finishes_empty() {
        let events = test_events();
        events.gathering_done.store(true, Ordering::SeqCst);
        let err = wait_first_relay(&events, Duration::from_secs(1))
            .await
            .unwrap_err();
        assert!(matches!(err, CoreError::Call(msg) if msg.contains("no relay")));
    }

    #[test]
    fn fingerprint_normalizes_colons() {
        assert!(fingerprints_match("sha-256 AA:BB:CC", "sha-256 aabbcc"));
        assert!(!fingerprints_match("sha-256 aa", "sha-256 ab"));
    }

    #[test]
    fn signaling_rejects_host_ice() {
        let msg = AppMessage {
            header: AppHeader {
                conv_seq: 1,
                sent_at: 1,
                reply_to: None,
            },
            body: AppBody::CallInvite {
                call_id: [1; CALL_ID_LEN],
                sdp: "v=0".into(),
                dtls_fp: "sha-256 aa".into(),
                ice: vec!["candidate:1 1 udp 1 192.0.2.1 9 typ host".into()],
                expires_at: 2,
            },
        };
        assert!(matches!(encode(&msg), Err(CoreError::DirectIceForbidden)));
    }

    fn docker_coturn() -> Option<(String, String)> {
        let name = format!("nemo-turn-{}", std::process::id());
        let port = 13_478u16;
        let ok = std::process::Command::new("docker")
            .args([
                "run",
                "-d",
                "--rm",
                "--network",
                "host",
                "--name",
                &name,
                "coturn/coturn:4.6-alpine",
                "-n",
                "--log-file=stdout",
                "--listening-port",
                &port.to_string(),
                "--lt-cred-mech",
                "--user=nemo:nemo",
                "--realm=nemo.local",
                "--no-cli",
                "--no-tls",
                "--no-dtls",
                "--listening-ip=127.0.0.1",
                "--relay-ip=127.0.0.1",
                "--external-ip=127.0.0.1",
                "--min-port=49152",
                "--max-port=49160",
                "--fingerprint",
                "--allow-loopback-peers",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .ok()?
            .success();
        if !ok {
            return None;
        }
        std::thread::sleep(Duration::from_secs(2));
        Some((format!("turn:127.0.0.1:{port}"), name))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn relayed_audio_through_local_coturn() {
        let Some((url, name)) = docker_coturn() else {
            eprintln!("skip I9 relay test: docker/coturn unavailable");
            return;
        };
        let turn = TurnConfig {
            url,
            username: "nemo".into(),
            credential: "nemo".into(),
            bind: "127.0.0.1:0".into(),
        };
        let result = async {
            let caller = Call::offer(&turn).await?;
            let callee = Call::answer(&turn, caller.local()).await?;
            caller.apply_answer(callee.local()).await?;
            caller.wait_connected().await?;
            callee.wait_connected().await?;
            caller.send_silence_frames(40).await?;
            callee.send_silence_frames(40).await?;
            let mut n = 0u64;
            for _ in 0..50 {
                n = caller.received_rtp() + callee.received_rtp();
                if n > 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(40)).await;
            }
            let _ = caller.close().await;
            let _ = callee.close().await;
            if n == 0 {
                return Err(CoreError::Call("no RTP through TURN".into()));
            }
            Ok(())
        }
        .await;
        let _ = std::process::Command::new("docker")
            .args(["rm", "-f", &name])
            .status();
        result.unwrap();
    }
}
