use nemo_core::app::{decode_text, encode_text};
use nemo_core::group::Group;
use nemo_core::mailbox;
use nemo_core::{CoreError, Installation};
use nemo_wire::envelope::{MessageType, TtlBucket, INNER_TEXT_OUTER};
use nemo_wire::hpke::HpkeKeypair;
use rand::RngCore;

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    futures::executor::block_on(f)
}

fn random_capability() -> [u8; 32] {
    mailbox::random_token()
}

fn two_member_group() -> (Installation, Installation, Group, Group) {
    let (alice, _) = Installation::create().unwrap();
    let (bob, _) = Installation::create().unwrap();
    let mut alice_group = alice.create_group().unwrap();
    let mut reserved = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut reserved);
    let pending = bob.prepare_join(reserved).unwrap();
    let (_commit, welcome) = alice_group
        .admit(alice.mls_provider(), &pending.key_package)
        .unwrap();
    let bob_group = pending.join(bob.mls_provider(), &welcome).unwrap();
    (alice, bob, alice_group, bob_group)
}

#[test]
fn one_to_one_through_mailbox_envelope() {
    let (mut alice, _) = Installation::create().unwrap();
    let (mut bob, _) = Installation::create().unwrap();
    let bob_server = HpkeKeypair::generate();
    let cap = random_capability();
    let bob_card = bob
        .mint_card(bob_server.public, "bob.example", 1_800_000_000)
        .unwrap();
    let bob_prekey = bob.mint_prekey().unwrap();
    block_on(alice.start_session(&bob_card, &bob_prekey, 1_700_000_000)).unwrap();

    let ptext = encode_text(1, 1_700_000_010, "hello bob").unwrap();
    let outer = block_on(alice.encrypt_to_mailbox(
        &bob.identity_id(),
        &bob_card.binding.server_hpke_public_key,
        cap,
        TtlBucket::DEFAULT,
        &ptext,
    ))
    .unwrap();
    assert_eq!(outer.destination_server_id, bob_server.server_id());
    let wire = outer.encode();
    assert_eq!(wire[0], 1);
    // version || dest_server_id — no sender identity
    assert_eq!(&wire[1..33], bob_server.server_id().as_slice());

    let inner = mailbox::deliver(&bob_server, &outer).unwrap();
    assert_eq!(inner.delivery_capability, cap);
    assert_eq!(inner.padded_message.type_, MessageType::DoubleRatchet);
    let opened = block_on(bob.decrypt_from_mailbox(&alice.identity_id(), &inner)).unwrap();
    assert_eq!(decode_text(&opened).unwrap(), (1, "hello bob".into()));
}

#[test]
fn dummy_and_real_text_share_outer_bucket() {
    let server = HpkeKeypair::generate();
    let cap = random_capability();
    let dummy = mailbox::wrap(
        &server.public,
        cap,
        TtlBucket::DEFAULT,
        MessageType::DoubleRatchet,
        vec![],
    )
    .unwrap();
    let real = mailbox::wrap(
        &server.public,
        cap,
        TtlBucket::DEFAULT,
        MessageType::DoubleRatchet,
        vec![1, 2, 3],
    )
    .unwrap();
    let dummy_inner = mailbox::deliver(&server, &dummy).unwrap();
    let real_inner = mailbox::deliver(&server, &real).unwrap();
    assert_eq!(dummy_inner.encode_padded().unwrap().len(), INNER_TEXT_OUTER);
    assert_eq!(real_inner.encode_padded().unwrap().len(), INNER_TEXT_OUTER);
    assert_eq!(dummy.hpke_ciphertext.len(), real.hpke_ciphertext.len());
}

#[test]
fn wrong_server_key_cannot_open() {
    let a = HpkeKeypair::generate();
    let b = HpkeKeypair::generate();
    let outer = mailbox::wrap(
        &a.public,
        random_capability(),
        TtlBucket::HOUR,
        MessageType::MlsApp,
        vec![9],
    )
    .unwrap();
    assert!(mailbox::deliver(&b, &outer).is_err());
}

#[test]
fn mls_app_through_mailbox() {
    let (alice, bob, mut alice_group, mut bob_group) = two_member_group();
    let bob_server = HpkeKeypair::generate();
    let cap = random_capability();
    let ptext = encode_text(3, 1_700_000_020, "hello group").unwrap();
    let outer = alice_group
        .encrypt_to_mailbox(
            alice.mls_provider(),
            &bob_server.public,
            cap,
            TtlBucket::DEFAULT,
            &ptext,
        )
        .unwrap();
    let inner = mailbox::deliver(&bob_server, &outer).unwrap();
    assert_eq!(inner.padded_message.type_, MessageType::MlsApp);
    let opened = bob_group
        .decrypt_from_mailbox(bob.mls_provider(), &inner)
        .unwrap();
    assert_eq!(decode_text(&opened).unwrap(), (3, "hello group".into()));
}

#[test]
fn welcome_through_mailbox() {
    let (alice, _) = Installation::create().unwrap();
    let (bob, _) = Installation::create().unwrap();
    let bob_server = HpkeKeypair::generate();
    let cap = random_capability();
    let mut alice_group = alice.create_group().unwrap();
    let mut reserved = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut reserved);
    let pending = bob.prepare_join(reserved).unwrap();
    let (_commit, welcome) = alice_group
        .admit(alice.mls_provider(), &pending.key_package)
        .unwrap();
    let outer =
        Group::wrap_handshake(&bob_server.public, cap, TtlBucket::DEFAULT, welcome).unwrap();
    let inner = mailbox::deliver(&bob_server, &outer).unwrap();
    assert_eq!(inner.padded_message.type_, MessageType::MlsHandshake);
    let body = mailbox::expect_type(&inner, MessageType::MlsHandshake).unwrap();
    let bob_group = pending.join(bob.mls_provider(), body).unwrap();
    assert_eq!(bob_group.member_count(), 2);
}

#[test]
fn remove_bundle_through_mailbox() {
    let (alice, bob, mut alice_group, mut bob_group) = two_member_group();
    let bob_server = HpkeKeypair::generate();
    let cap = random_capability();
    let bundle = alice_group
        .remove(alice.mls_provider(), bob_group.credential_id())
        .unwrap();
    let outer = Group::wrap_remove(&bob_server.public, cap, TtlBucket::DEFAULT, &bundle).unwrap();
    let inner = mailbox::deliver(&bob_server, &outer).unwrap();
    assert_eq!(inner.padded_message.type_, MessageType::RemoveBundle);
    bob_group
        .apply_remove_from_mailbox(bob.mls_provider(), &inner)
        .unwrap();
    assert_eq!(bob_group.member_count(), 1);
}

#[test]
fn one_to_one_refuses_mls_inner() {
    let (mut alice, _) = Installation::create().unwrap();
    let (mut bob, _) = Installation::create().unwrap();
    let server = HpkeKeypair::generate();
    let bob_card = bob
        .mint_card(server.public, "bob.example", 1_800_000_000)
        .unwrap();
    block_on(alice.start_session(&bob_card, &bob.mint_prekey().unwrap(), 1_700_000_000)).unwrap();
    let inner = mailbox::deliver(
        &server,
        &mailbox::wrap(
            &server.public,
            random_capability(),
            TtlBucket::DEFAULT,
            MessageType::MlsApp,
            vec![1],
        )
        .unwrap(),
    )
    .unwrap();
    let err = block_on(bob.decrypt_from_mailbox(&alice.identity_id(), &inner)).unwrap_err();
    assert!(matches!(err, CoreError::WrongMailboxType));
}

#[test]
fn attachment_dr_uses_a_star_bucket() {
    let (mut alice, _) = Installation::create().unwrap();
    let (mut bob, _) = Installation::create().unwrap();
    let bob_server = HpkeKeypair::generate();
    let cap = random_capability();
    let bob_card = bob
        .mint_card(bob_server.public, "bob.example", 1_800_000_000)
        .unwrap();
    block_on(alice.start_session(&bob_card, &bob.mint_prekey().unwrap(), 1_700_000_000)).unwrap();
    let ptext = encode_text(1, 1_700_000_010, "file-bytes").unwrap();
    let outer = block_on(alice.encrypt_attachment_to_mailbox(
        &bob.identity_id(),
        &bob_card.binding.server_hpke_public_key,
        cap,
        TtlBucket::DEFAULT,
        &ptext,
    ))
    .unwrap();
    let inner = mailbox::deliver(&bob_server, &outer).unwrap();
    assert_eq!(inner.padded_message.type_, MessageType::AttachmentDr);
    assert!(inner.encode_padded().unwrap().len() >= nemo_wire::envelope::A1_OUTER);
    let opened = block_on(bob.decrypt_from_mailbox(&alice.identity_id(), &inner)).unwrap();
    assert_eq!(decode_text(&opened).unwrap(), (1, "file-bytes".into()));
}

#[test]
fn mailbox_seq_gap_is_loss() {
    assert!(!nemo_core::messages_lost(3, 4));
    assert!(nemo_core::messages_lost(3, 5));
    assert!(!nemo_core::messages_lost(0, 1));
}
