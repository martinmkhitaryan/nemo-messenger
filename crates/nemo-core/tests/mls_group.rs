use std::time::{Duration, SystemTime};

use nemo_core::group::{Group, UPDATE_BEFORE_SEND};
use nemo_core::CoreError;
use nemo_core::{decode_text, encode_text, Installation};
use rand::RngCore;

fn random_credential_id() -> [u8; 32] {
    let mut id = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut id);
    id
}

fn two_member_group() -> (Installation, Installation, Group, Group) {
    let (alice, _) = Installation::create().unwrap();
    let (bob, _) = Installation::create().unwrap();
    let mut alice_group = alice.create_group().unwrap();
    let reserved = random_credential_id();
    let pending = bob.prepare_join(reserved).unwrap();
    let (_commit, welcome) = alice_group
        .admit(alice.mls_provider(), &pending.key_package)
        .unwrap();
    let bob_group = pending.join(bob.mls_provider(), &welcome).unwrap();
    assert_eq!(alice_group.member_count(), 2);
    assert_eq!(bob_group.member_count(), 2);
    assert_eq!(bob_group.credential_id(), reserved);
    (alice, bob, alice_group, bob_group)
}

#[test]
fn suite_is_rfc_9420_0x0003() {
    assert_eq!(u16::from(nemo_core::CIPHERSUITE), 0x0003);
}

#[test]
fn create_admit_app_message() {
    let (alice, bob, mut alice_group, mut bob_group) = two_member_group();
    let ptext = encode_text(1, 1_700_000_000, "hello group").unwrap();
    let ctext = alice_group.encrypt(alice.mls_provider(), &ptext).unwrap();
    let opened = bob_group.decrypt(bob.mls_provider(), &ctext).unwrap();
    assert_eq!(decode_text(&opened).unwrap(), (1, "hello group".into()));

    let reply = encode_text(2, 1_700_000_001, "hi alice").unwrap();
    let reply_ct = bob_group.encrypt(bob.mls_provider(), &reply).unwrap();
    let reply_pt = alice_group
        .decrypt(alice.mls_provider(), &reply_ct)
        .unwrap();
    assert_eq!(decode_text(&reply_pt).unwrap(), (2, "hi alice".into()));
}

#[test]
fn refuse_send_if_update_older_than_72h() {
    let (alice, _bob, mut alice_group, _bob_group) = two_member_group();
    alice_group.set_last_own_update_for_test(
        SystemTime::now() - UPDATE_BEFORE_SEND - Duration::from_secs(1),
    );
    let err = alice_group
        .encrypt(alice.mls_provider(), b"late")
        .unwrap_err();
    assert!(matches!(err, CoreError::StaleMlsUpdate));
    let _ = alice_group.self_update(alice.mls_provider()).unwrap();
    alice_group.encrypt(alice.mls_provider(), b"ok").unwrap();
}

#[test]
fn remove_bundle_ejects_named_credential() {
    let (alice, bob, mut alice_group, mut bob_group) = two_member_group();
    let bob_id = bob_group.credential_id();
    let bundle = alice_group.remove(alice.mls_provider(), bob_id).unwrap();
    bob_group
        .apply_remove_bundle(bob.mls_provider(), &bundle)
        .unwrap();
    assert_eq!(alice_group.member_count(), 1);
    assert_eq!(bob_group.member_count(), 1);
}

#[test]
fn unframed_remove_is_rejected() {
    let (alice, bob, mut alice_group, mut bob_group) = two_member_group();
    let bob_id = bob_group.credential_id();
    let bundle = alice_group.remove(alice.mls_provider(), bob_id).unwrap();
    let err = bob_group
        .apply_handshake(bob.mls_provider(), &bundle.mls_commit)
        .unwrap_err();
    assert!(matches!(err, CoreError::UnframedRemove));
}
