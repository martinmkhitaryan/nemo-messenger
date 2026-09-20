use nemo_core::app::{decode_text, encode_text};
use nemo_core::identity::{revocation_from_mnemonic, Installation};
use nemo_core::CoreError;
use nemo_wire::revocation::RevocationStatement;

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    futures::executor::block_on(f)
}

#[test]
fn revocation_mnemonic_is_not_stored_and_verifies() {
    let (inst, export) = Installation::create().unwrap();
    assert_eq!(export.mnemonic.split_whitespace().count(), 24);
    let stmt =
        revocation_from_mnemonic(&export.mnemonic, inst.identity_id(), 1_700_000_000).unwrap();
    let pk = nemo_wire::VerifyingKey::from_bytes(&export.public_key).unwrap();
    stmt.verify(&pk).unwrap();
    assert_eq!(stmt.identity_id, inst.identity_id());
    assert!(RevocationStatement::decode(&stmt.encode())
        .unwrap()
        .verify(&inst.verifying_key())
        .is_err());
}

#[test]
fn each_card_has_a_fresh_share_token() {
    let (mut inst, _) = Installation::create().unwrap();
    let hpke = [7u8; 32];
    let a = inst.mint_card(hpke, "nemo.example", 1_800_000_000).unwrap();
    let b = inst.mint_card(hpke, "nemo.example", 1_800_000_000).unwrap();
    a.verify(1_700_000_000).unwrap();
    assert_ne!(a.share_token, b.share_token);
    assert_eq!(b.binding.seq, 2);
}

#[test]
fn alice_bob_pqxdh_then_ratchet() {
    let (mut alice, _) = Installation::create().unwrap();
    let (mut bob, _) = Installation::create().unwrap();
    let hpke = [9u8; 32];
    let bob_card = bob.mint_card(hpke, "bob.example", 1_800_000_000).unwrap();
    let bob_prekey = bob.mint_prekey().unwrap();

    block_on(alice.start_session(&bob_card, &bob_prekey, 1_700_000_000)).unwrap();

    let ptext = encode_text(1, 1_700_000_010, "hello bob").unwrap();
    let ctext = block_on(alice.encrypt(&bob.identity_id(), &ptext)).unwrap();
    let opened = block_on(bob.decrypt(&alice.identity_id(), &ctext)).unwrap();
    assert_eq!(decode_text(&opened).unwrap(), (1, "hello bob".into()));

    let reply = encode_text(2, 1_700_000_011, "hello alice").unwrap();
    let reply_ct = block_on(bob.encrypt(&alice.identity_id(), &reply)).unwrap();
    let reply_pt = block_on(alice.decrypt(&bob.identity_id(), &reply_ct)).unwrap();
    assert_eq!(decode_text(&reply_pt).unwrap(), (2, "hello alice".into()));
}

#[test]
fn refuses_prekey_signed_by_someone_else() {
    let (mut alice, _) = Installation::create().unwrap();
    let (mut bob, _) = Installation::create().unwrap();
    let (mut mallory, _) = Installation::create().unwrap();
    let hpke = [1u8; 32];
    let bob_card = bob.mint_card(hpke, "bob.example", 1_800_000_000).unwrap();
    let mallory_prekey = mallory.mint_prekey().unwrap();
    let err = block_on(alice.start_session(&bob_card, &mallory_prekey, 1_700_000_000)).unwrap_err();
    assert!(matches!(
        err,
        CoreError::Wire(_) | CoreError::IdentityMismatch
    ));
}
