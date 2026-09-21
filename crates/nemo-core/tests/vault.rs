use std::fs;
use std::path::PathBuf;

use nemo_core::app::{decode_text, encode_text};
use nemo_core::identity::Installation;
use nemo_core::{CoreError, Vault};
use rand::RngCore;

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    futures::executor::block_on(f)
}

fn temp_dir() -> PathBuf {
    let mut n = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut n);
    let dir = std::env::temp_dir().join(format!("nemo-vault-{}", nemo_wire::ids::to_hex(&n)));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn snapshot_roundtrip_keeps_ratchet() {
    let (mut alice, export) = Installation::create().unwrap();
    let (mut bob, _) = Installation::create().unwrap();
    let hpke = [3u8; 32];
    let bob_card = bob.mint_card(hpke, "bob.example", 1_800_000_000).unwrap();
    let bob_prekey = bob.mint_prekey().unwrap();
    block_on(alice.start_session(&bob_card, &bob_prekey, 1_700_000_000)).unwrap();
    let ptext = encode_text(1, 1_700_000_010, "hello bob").unwrap();
    let ctext = block_on(alice.encrypt(&bob.identity_id(), &ptext)).unwrap();

    let restored = Installation::decode_snapshot(&alice.encode_snapshot().unwrap()).unwrap();
    assert_eq!(restored.identity_id(), alice.identity_id());
    assert_eq!(
        restored.revocation_public_key(),
        alice.revocation_public_key()
    );
    assert!(!alice
        .encode_snapshot()
        .unwrap()
        .windows(export.mnemonic.len())
        .any(|w| w == export.mnemonic.as_bytes()));

    let opened = block_on(bob.decrypt(&alice.identity_id(), &ctext)).unwrap();
    assert_eq!(decode_text(&opened).unwrap(), (1, "hello bob".into()));

    let reply = encode_text(2, 1_700_000_011, "hello alice").unwrap();
    let reply_ct = block_on(bob.encrypt(&alice.identity_id(), &reply)).unwrap();
    let mut alice2 = restored;
    let reply_pt = block_on(alice2.decrypt(&bob.identity_id(), &reply_ct)).unwrap();
    assert_eq!(decode_text(&reply_pt).unwrap(), (2, "hello alice".into()));
}

#[test]
fn sqlcipher_open_wrong_passphrase_fails() {
    let dir = temp_dir();
    let (install, _) = Installation::create().unwrap();
    Vault::create(&dir, "correct horse", &install).unwrap();
    let err = match Vault::open(&dir, "incorrect!!") {
        Ok(_) => panic!("wrong passphrase opened the vault"),
        Err(e) => e,
    };
    assert!(matches!(
        err,
        CoreError::VaultLocked | CoreError::VaultIo(_)
    ));
    let db = fs::read(dir.join("store.db")).unwrap();
    let pk = install.revocation_public_key();
    assert!(
        !db.windows(pk.len()).any(|w| w == pk),
        "revocation public key must not appear in the ciphertext file"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn stolen_vault_without_passphrase_does_not_unlock() {
    let dir = temp_dir();
    let (install, _) = Installation::create().unwrap();
    Vault::create(&dir, "correct horse", &install).unwrap();
    assert!(matches!(
        Vault::open(&dir, "not the passphrase!!"),
        Err(CoreError::VaultLocked | CoreError::VaultIo(_))
    ));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn sqlcipher_roundtrip_and_refuse_short_passphrase() {
    let dir = temp_dir();
    let (mut alice, _) = Installation::create().unwrap();
    let (mut bob, _) = Installation::create().unwrap();
    let hpke = [9u8; 32];
    let bob_card = bob.mint_card(hpke, "bob.example", 1_800_000_000).unwrap();
    let bob_prekey = bob.mint_prekey().unwrap();
    block_on(alice.start_session(&bob_card, &bob_prekey, 1_700_000_000)).unwrap();

    assert!(matches!(
        Vault::create(&dir, "short", &alice),
        Err(CoreError::WeakPassphrase)
    ));

    let vault = Vault::create(&dir, "correct horse", &alice).unwrap();
    drop(vault);
    let (_vault, mut loaded) = Vault::open(&dir, "correct horse").unwrap();
    assert_eq!(loaded.identity_id(), alice.identity_id());

    let ptext = encode_text(1, 1_700_000_010, "ping").unwrap();
    let ctext = block_on(loaded.encrypt(&bob.identity_id(), &ptext)).unwrap();
    let opened = block_on(bob.decrypt(&alice.identity_id(), &ctext)).unwrap();
    assert_eq!(decode_text(&opened).unwrap(), (1, "ping".into()));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn vault_needs_matching_device_secret() {
    let dir = temp_dir();
    let (install, _) = Installation::create().unwrap();
    let secret = [7u8; 32];
    Vault::create_bound(&dir, "correct horse", &install, Some(&secret)).unwrap();
    assert!(matches!(
        Vault::open_bound(&dir, "correct horse", Some(&[8u8; 32])),
        Err(CoreError::VaultLocked | CoreError::VaultIo(_) | CoreError::VaultCorrupt)
    ));
    let (_vault, loaded) = Vault::open_bound(&dir, "correct horse", Some(&secret)).unwrap();
    assert_eq!(loaded.identity_id(), install.identity_id());
    let _ = fs::remove_dir_all(&dir);
}
