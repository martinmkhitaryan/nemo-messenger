//! Phase-7 application plaintext. Servers never see these fields.
//!
//! 1:1 call signaling types ship. Group calls / SFrame are nice-to-have.

use nemo_wire::cbor::{self, Value};
use nemo_wire::envelope::TtlBucket;
use nemo_wire::ids::{copy_fixed, IdentityId, ServerId, KEY_LEN};
use nemo_wire::PROTOCOL_VERSION;
use rand::RngCore;

use crate::error::{CoreError, Result};

pub const TEXT_MAX_BYTES: usize = 8192;
pub const EMOJI_MAX_BYTES: usize = 32;
pub const CALL_ID_LEN: usize = 16;
pub const CONTACT_CAP_LEN: usize = KEY_LEN;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppHeader {
    pub conv_seq: u64,
    pub sent_at: u64,
    pub reply_to: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMeta {
    pub name: String,
    pub mime: String,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppBody {
    Text {
        text: String,
    },
    Attachment {
        enc_file: Vec<u8>,
        meta: FileMeta,
        /// Groups only: 32-byte host fetch token.
        fetch_token: Option<[u8; KEY_LEN]>,
    },
    Reaction {
        target: u64,
        emoji: String,
    },
    Delete {
        target: u64,
    },
    ProtocolAck {
        upto: u64,
    },
    Capability {
        contact_capability: [u8; CONTACT_CAP_LEN],
    },
    BindingGossip {
        identity_id: IdentityId,
        seq: u64,
        server_id: ServerId,
    },
    Disappear {
        seconds: u64,
    },
    CallInvite {
        call_id: [u8; CALL_ID_LEN],
        sdp: String,
        dtls_fp: String,
        ice: Vec<String>,
        expires_at: u64,
    },
    CallRinging {
        call_id: [u8; CALL_ID_LEN],
    },
    CallAnswer {
        call_id: [u8; CALL_ID_LEN],
        sdp: String,
        dtls_fp: String,
        ice: Vec<String>,
    },
    CallIce {
        call_id: [u8; CALL_ID_LEN],
        ice: Vec<String>,
    },
    CallReject {
        call_id: [u8; CALL_ID_LEN],
    },
    CallCancel {
        call_id: [u8; CALL_ID_LEN],
    },
    CallEnd {
        call_id: [u8; CALL_ID_LEN],
    },
    Unknown {
        msg_type: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppMessage {
    pub header: AppHeader,
    pub body: AppBody,
}

impl AppMessage {
    pub fn text(conv_seq: u64, sent_at: u64, text: impl Into<String>) -> Self {
        Self {
            header: AppHeader {
                conv_seq,
                sent_at,
                reply_to: None,
            },
            body: AppBody::Text { text: text.into() },
        }
    }

    pub fn msg_type(&self) -> &str {
        match &self.body {
            AppBody::Text { .. } => "text",
            AppBody::Attachment { .. } => "attachment",
            AppBody::Reaction { .. } => "reaction",
            AppBody::Delete { .. } => "delete",
            AppBody::ProtocolAck { .. } => "protocol_ack",
            AppBody::Capability { .. } => "capability",
            AppBody::BindingGossip { .. } => "binding_gossip",
            AppBody::Disappear { .. } => "disappear",
            AppBody::CallInvite { .. } => "call_invite",
            AppBody::CallRinging { .. } => "call_ringing",
            AppBody::CallAnswer { .. } => "call_answer",
            AppBody::CallIce { .. } => "call_ice",
            AppBody::CallReject { .. } => "call_reject",
            AppBody::CallCancel { .. } => "call_cancel",
            AppBody::CallEnd { .. } => "call_end",
            AppBody::Unknown { msg_type } => msg_type,
        }
    }
}

/// Stale call invites die with `ttl_bucket` 1 (60 s).
pub fn invite_ttl_bucket() -> TtlBucket {
    TtlBucket::SECONDS_60
}

pub fn random_call_id() -> [u8; CALL_ID_LEN] {
    let mut id = [0u8; CALL_ID_LEN];
    rand::rngs::OsRng.fill_bytes(&mut id);
    id
}

pub fn encode_text(conv_seq: u64, sent_at: u64, text: &str) -> Result<Vec<u8>> {
    encode(&AppMessage::text(conv_seq, sent_at, text))
}

pub fn decode_text(bytes: &[u8]) -> Result<(u64, String)> {
    let msg = decode(bytes)?;
    match msg.body {
        AppBody::Text { text } => Ok((msg.header.conv_seq, text)),
        _ => Err(CoreError::Wire(nemo_wire::WireError::Cbor("expected text"))),
    }
}

pub fn encode(msg: &AppMessage) -> Result<Vec<u8>> {
    if matches!(msg.body, AppBody::Unknown { .. }) {
        return Err(CoreError::Wire(nemo_wire::WireError::Cbor(
            "cannot encode unknown msg_type",
        )));
    }
    validate(msg)?;
    let mut pairs = vec![
        (0, Value::Uint(PROTOCOL_VERSION as u64)),
        (1, Value::Text(msg.msg_type().into())),
        (2, Value::Uint(msg.header.conv_seq)),
        (3, Value::Uint(msg.header.sent_at)),
    ];
    if let Some(reply_to) = msg.header.reply_to {
        pairs.push((4, Value::Uint(reply_to)));
    }
    extra_fields(&msg.body, &mut pairs);
    Ok(cbor::encode(&Value::Map(pairs)))
}

pub fn decode(bytes: &[u8]) -> Result<AppMessage> {
    let Value::Map(m) = cbor::decode(bytes).map_err(CoreError::from)? else {
        return Err(CoreError::Wire(nemo_wire::WireError::Cbor(
            "app message must be a map",
        )));
    };
    let version = cbor::expect_uint(cbor::map_get(&m, 0)?)?;
    if version != PROTOCOL_VERSION as u64 {
        return Err(CoreError::Wire(nemo_wire::WireError::UnknownVersion(
            version,
        )));
    }
    let msg_type = cbor::expect_text(cbor::map_get(&m, 1)?)?.to_owned();
    let header = AppHeader {
        conv_seq: cbor::expect_uint(cbor::map_get(&m, 2)?)?,
        sent_at: cbor::expect_uint(cbor::map_get(&m, 3)?)?,
        reply_to: match cbor::map_get_opt(&m, 4) {
            Some(v) => Some(cbor::expect_uint(v)?),
            None => None,
        },
    };
    let body = parse_body(&msg_type, &m)?;
    let msg = AppMessage { header, body };
    if !matches!(msg.body, AppBody::Unknown { .. }) {
        validate(&msg)?;
    }
    Ok(msg)
}

fn extra_fields(body: &AppBody, pairs: &mut Vec<(u64, Value)>) {
    match body {
        AppBody::Text { text } => pairs.push((5, Value::Text(text.clone()))),
        AppBody::Attachment {
            enc_file,
            meta,
            fetch_token,
        } => {
            pairs.push((5, Value::Bytes(enc_file.clone())));
            pairs.push((6, encode_meta(meta)));
            if let Some(token) = fetch_token {
                pairs.push((7, Value::Bytes(token.to_vec())));
            }
        }
        AppBody::Reaction { target, emoji } => {
            pairs.push((5, Value::Uint(*target)));
            pairs.push((6, Value::Text(emoji.clone())));
        }
        AppBody::Delete { target } => pairs.push((5, Value::Uint(*target))),
        AppBody::ProtocolAck { upto } => pairs.push((5, Value::Uint(*upto))),
        AppBody::Capability { contact_capability } => {
            pairs.push((5, Value::Bytes(contact_capability.to_vec())))
        }
        AppBody::BindingGossip {
            identity_id,
            seq,
            server_id,
        } => {
            pairs.push((5, Value::Bytes(identity_id.to_vec())));
            pairs.push((6, Value::Uint(*seq)));
            pairs.push((7, Value::Bytes(server_id.to_vec())));
        }
        AppBody::Disappear { seconds } => pairs.push((5, Value::Uint(*seconds))),
        AppBody::CallInvite {
            call_id,
            sdp,
            dtls_fp,
            ice,
            expires_at,
        } => {
            pairs.push((5, Value::Bytes(call_id.to_vec())));
            pairs.push((6, Value::Text(sdp.clone())));
            pairs.push((7, Value::Text(dtls_fp.clone())));
            pairs.push((8, ice_value(ice)));
            pairs.push((9, Value::Uint(*expires_at)));
        }
        AppBody::CallRinging { call_id } => pairs.push((5, Value::Bytes(call_id.to_vec()))),
        AppBody::CallAnswer {
            call_id,
            sdp,
            dtls_fp,
            ice,
        } => {
            pairs.push((5, Value::Bytes(call_id.to_vec())));
            pairs.push((6, Value::Text(sdp.clone())));
            pairs.push((7, Value::Text(dtls_fp.clone())));
            pairs.push((8, ice_value(ice)));
        }
        AppBody::CallIce { call_id, ice } => {
            pairs.push((5, Value::Bytes(call_id.to_vec())));
            pairs.push((6, ice_value(ice)));
        }
        AppBody::CallReject { call_id }
        | AppBody::CallCancel { call_id }
        | AppBody::CallEnd { call_id } => pairs.push((5, Value::Bytes(call_id.to_vec()))),
        AppBody::Unknown { .. } => {}
    }
}

fn parse_body(msg_type: &str, m: &[(u64, Value)]) -> Result<AppBody> {
    Ok(match msg_type {
        "text" => AppBody::Text {
            text: cbor::expect_text(cbor::map_get(m, 5)?)?.to_owned(),
        },
        "attachment" => AppBody::Attachment {
            enc_file: cbor::expect_bytes(cbor::map_get(m, 5)?)?.to_vec(),
            meta: decode_meta(cbor::map_get(m, 6)?)?,
            fetch_token: match cbor::map_get_opt(m, 7) {
                Some(v) => Some(fixed(cbor::expect_bytes(v)?)?),
                None => None,
            },
        },
        "reaction" => AppBody::Reaction {
            target: cbor::expect_uint(cbor::map_get(m, 5)?)?,
            emoji: cbor::expect_text(cbor::map_get(m, 6)?)?.to_owned(),
        },
        "delete" => AppBody::Delete {
            target: cbor::expect_uint(cbor::map_get(m, 5)?)?,
        },
        "protocol_ack" => AppBody::ProtocolAck {
            upto: cbor::expect_uint(cbor::map_get(m, 5)?)?,
        },
        "capability" => AppBody::Capability {
            contact_capability: fixed(cbor::expect_bytes(cbor::map_get(m, 5)?)?)?,
        },
        "binding_gossip" => AppBody::BindingGossip {
            identity_id: fixed(cbor::expect_bytes(cbor::map_get(m, 5)?)?)?,
            seq: cbor::expect_uint(cbor::map_get(m, 6)?)?,
            server_id: fixed(cbor::expect_bytes(cbor::map_get(m, 7)?)?)?,
        },
        "disappear" => AppBody::Disappear {
            seconds: cbor::expect_uint(cbor::map_get(m, 5)?)?,
        },
        "call_invite" => AppBody::CallInvite {
            call_id: call_id(cbor::map_get(m, 5)?)?,
            sdp: cbor::expect_text(cbor::map_get(m, 6)?)?.to_owned(),
            dtls_fp: cbor::expect_text(cbor::map_get(m, 7)?)?.to_owned(),
            ice: decode_ice(cbor::map_get(m, 8)?)?,
            expires_at: cbor::expect_uint(cbor::map_get(m, 9)?)?,
        },
        "call_ringing" => AppBody::CallRinging {
            call_id: call_id(cbor::map_get(m, 5)?)?,
        },
        "call_answer" => AppBody::CallAnswer {
            call_id: call_id(cbor::map_get(m, 5)?)?,
            sdp: cbor::expect_text(cbor::map_get(m, 6)?)?.to_owned(),
            dtls_fp: cbor::expect_text(cbor::map_get(m, 7)?)?.to_owned(),
            ice: decode_ice(cbor::map_get(m, 8)?)?,
        },
        "call_ice" => AppBody::CallIce {
            call_id: call_id(cbor::map_get(m, 5)?)?,
            ice: decode_ice(cbor::map_get(m, 6)?)?,
        },
        "call_reject" => AppBody::CallReject {
            call_id: call_id(cbor::map_get(m, 5)?)?,
        },
        "call_cancel" => AppBody::CallCancel {
            call_id: call_id(cbor::map_get(m, 5)?)?,
        },
        "call_end" => AppBody::CallEnd {
            call_id: call_id(cbor::map_get(m, 5)?)?,
        },
        other => AppBody::Unknown {
            msg_type: other.to_owned(),
        },
    })
}

fn validate(msg: &AppMessage) -> Result<()> {
    match &msg.body {
        AppBody::Text { text } => {
            if text.len() > TEXT_MAX_BYTES {
                return Err(CoreError::TextTooLong);
            }
        }
        AppBody::Reaction { emoji, .. } => {
            if emoji.len() > EMOJI_MAX_BYTES {
                return Err(CoreError::EmojiTooLong);
            }
        }
        AppBody::CallInvite { ice, .. }
        | AppBody::CallAnswer { ice, .. }
        | AppBody::CallIce { ice, .. } => reject_direct_ice(ice)?,
        _ => {}
    }
    Ok(())
}

fn encode_meta(meta: &FileMeta) -> Value {
    Value::Map(vec![
        (0, Value::Text(meta.name.clone())),
        (1, Value::Text(meta.mime.clone())),
        (2, Value::Uint(meta.size)),
    ])
}

fn decode_meta(v: &Value) -> Result<FileMeta> {
    let m = cbor::expect_map(v)?;
    Ok(FileMeta {
        name: cbor::expect_text(cbor::map_get(m, 0)?)?.to_owned(),
        mime: cbor::expect_text(cbor::map_get(m, 1)?)?.to_owned(),
        size: cbor::expect_uint(cbor::map_get(m, 2)?)?,
    })
}

fn ice_value(ice: &[String]) -> Value {
    Value::Array(ice.iter().cloned().map(Value::Text).collect())
}

fn decode_ice(v: &Value) -> Result<Vec<String>> {
    let items = cbor::expect_array(v)?;
    let mut ice = Vec::with_capacity(items.len());
    for item in items {
        ice.push(cbor::expect_text(item)?.to_owned());
    }
    Ok(ice)
}

pub fn reject_direct_ice(ice: &[String]) -> Result<()> {
    for c in ice {
        let lower = c.to_ascii_lowercase();
        if ice_typ(&lower, "host") || ice_typ(&lower, "srflx") {
            return Err(CoreError::DirectIceForbidden);
        }
    }
    Ok(())
}

fn ice_typ(candidate: &str, typ: &str) -> bool {
    let mut prev = "";
    for tok in candidate.split_whitespace() {
        if prev == "typ" && tok == typ {
            return true;
        }
        prev = tok;
    }
    false
}

fn call_id(v: &Value) -> Result<[u8; CALL_ID_LEN]> {
    fixed(cbor::expect_bytes(v)?)
}

fn fixed<const N: usize>(bytes: &[u8]) -> Result<[u8; N]> {
    Ok(copy_fixed(bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(seq: u64) -> AppHeader {
        AppHeader {
            conv_seq: seq,
            sent_at: 1_700_000_000,
            reply_to: None,
        }
    }

    fn roundtrip(body: AppBody) -> AppMessage {
        let msg = AppMessage {
            header: header(1),
            body,
        };
        decode(&encode(&msg).unwrap()).unwrap()
    }

    fn relay(ip: &str) -> String {
        format!("candidate:1 1 udp 1 {ip} 3478 typ relay")
    }

    #[test]
    fn text_roundtrip() {
        let bytes = encode_text(1, 1_700_000_000, "hello").unwrap();
        assert_eq!(decode_text(&bytes).unwrap(), (1, "hello".into()));
        let msg = decode(&bytes).unwrap();
        assert_eq!(msg.header.reply_to, None);
        assert_eq!(
            msg.body,
            AppBody::Text {
                text: "hello".into()
            }
        );
    }

    #[test]
    fn text_reply_to_and_too_long() {
        let msg = AppMessage {
            header: AppHeader {
                conv_seq: 2,
                sent_at: 1,
                reply_to: Some(1),
            },
            body: AppBody::Text { text: "re".into() },
        };
        let out = decode(&encode(&msg).unwrap()).unwrap();
        assert_eq!(out.header.reply_to, Some(1));
        let long = "x".repeat(TEXT_MAX_BYTES + 1);
        assert!(matches!(
            encode_text(1, 0, &long),
            Err(CoreError::TextTooLong)
        ));
    }

    #[test]
    fn attachment_1to1_and_group() {
        let meta = FileMeta {
            name: "a.bin".into(),
            mime: "application/octet-stream".into(),
            size: 3,
        };
        let one = roundtrip(AppBody::Attachment {
            enc_file: vec![1, 2, 3],
            meta: meta.clone(),
            fetch_token: None,
        });
        assert!(matches!(
            one.body,
            AppBody::Attachment {
                fetch_token: None,
                ..
            }
        ));
        let token = [7u8; KEY_LEN];
        let group = roundtrip(AppBody::Attachment {
            enc_file: vec![],
            meta,
            fetch_token: Some(token),
        });
        match group.body {
            AppBody::Attachment { fetch_token, .. } => assert_eq!(fetch_token, Some(token)),
            _ => panic!("attachment"),
        }
    }

    #[test]
    fn reaction_delete_ack_capability_gossip_disappear() {
        assert_eq!(
            roundtrip(AppBody::Reaction {
                target: 4,
                emoji: "👍".into(),
            })
            .body,
            AppBody::Reaction {
                target: 4,
                emoji: "👍".into(),
            }
        );
        assert!(matches!(
            encode(&AppMessage {
                header: header(1),
                body: AppBody::Reaction {
                    target: 1,
                    emoji: "x".repeat(EMOJI_MAX_BYTES + 1),
                },
            }),
            Err(CoreError::EmojiTooLong)
        ));
        assert_eq!(
            roundtrip(AppBody::Delete { target: 9 }).body,
            AppBody::Delete { target: 9 }
        );
        assert_eq!(
            roundtrip(AppBody::ProtocolAck { upto: 12 }).body,
            AppBody::ProtocolAck { upto: 12 }
        );
        let cap = [3u8; CONTACT_CAP_LEN];
        assert_eq!(
            roundtrip(AppBody::Capability {
                contact_capability: cap,
            })
            .body,
            AppBody::Capability {
                contact_capability: cap,
            }
        );
        let gossip = AppBody::BindingGossip {
            identity_id: [1u8; KEY_LEN],
            seq: 4,
            server_id: [2u8; KEY_LEN],
        };
        assert_eq!(roundtrip(gossip.clone()).body, gossip);
        assert_eq!(
            roundtrip(AppBody::Disappear { seconds: 0 }).body,
            AppBody::Disappear { seconds: 0 }
        );
    }

    #[test]
    fn call_signaling_relay_only() {
        let id = random_call_id();
        let ice = vec![relay("203.0.113.1")];
        let invite = AppMessage {
            header: header(1),
            body: AppBody::CallInvite {
                call_id: id,
                sdp: "v=0".into(),
                dtls_fp: "sha-256 AA".into(),
                ice: ice.clone(),
                expires_at: 1_700_000_060,
            },
        };
        assert_eq!(invite_ttl_bucket(), TtlBucket::SECONDS_60);
        assert_eq!(decode(&encode(&invite).unwrap()).unwrap(), invite);
        assert_eq!(
            roundtrip(AppBody::CallRinging { call_id: id }).body,
            AppBody::CallRinging { call_id: id }
        );
        assert_eq!(
            roundtrip(AppBody::CallAnswer {
                call_id: id,
                sdp: "v=0".into(),
                dtls_fp: "sha-256 BB".into(),
                ice: ice.clone(),
            })
            .body,
            AppBody::CallAnswer {
                call_id: id,
                sdp: "v=0".into(),
                dtls_fp: "sha-256 BB".into(),
                ice,
            }
        );
        assert_eq!(
            roundtrip(AppBody::CallIce {
                call_id: id,
                ice: vec![relay("198.51.100.2")],
            })
            .msg_type(),
            "call_ice"
        );
        assert_eq!(
            roundtrip(AppBody::CallReject { call_id: id }).body,
            AppBody::CallReject { call_id: id }
        );
        assert_eq!(
            roundtrip(AppBody::CallCancel { call_id: id }).body,
            AppBody::CallCancel { call_id: id }
        );
        assert_eq!(
            roundtrip(AppBody::CallEnd { call_id: id }).body,
            AppBody::CallEnd { call_id: id }
        );
    }

    #[test]
    fn rejects_host_and_srflx_ice() {
        let id = [9u8; CALL_ID_LEN];
        for typ in ["host", "srflx"] {
            let msg = AppMessage {
                header: header(1),
                body: AppBody::CallIce {
                    call_id: id,
                    ice: vec![format!("candidate:1 1 udp 1 192.0.2.1 9 typ {typ}")],
                },
            };
            assert!(
                matches!(encode(&msg), Err(CoreError::DirectIceForbidden)),
                "{typ}"
            );
        }
    }

    #[test]
    fn unknown_msg_type_is_ignored() {
        let bytes = cbor::encode(&Value::Map(vec![
            (0, Value::Uint(1)),
            (1, Value::Text("future_type".into())),
            (2, Value::Uint(1)),
            (3, Value::Uint(0)),
        ]));
        let msg = decode(&bytes).unwrap();
        assert_eq!(
            msg.body,
            AppBody::Unknown {
                msg_type: "future_type".into()
            }
        );
        assert!(decode_text(&bytes).is_err());
        assert!(encode(&msg).is_err());
    }
}
