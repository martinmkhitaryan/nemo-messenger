use nemo_server::federation::Enqueue;
use nemo_server::group::{FanoutTarget, GroupHost};
use nemo_server::home::{HomeServer, Limits};
use nemo_server::ServerError;
use nemo_wire::envelope::{InnerEnvelope, MessageType, PaddedMessage, TtlBucket};
use nemo_wire::hpke::seal_to_server;
use nemo_wire::ids::KEY_LEN;
use nemo_wire::{
    identity_id, AttachmentReserve, AttachmentSizeBucket, ContactCard, GroupAdmit, GroupInvite,
    HomeServerBinding, MailboxOwnerAuth, RemoveBundle, RevocationStatement, SigningKey,
    INTRO_TTL_30_MIN,
};
use rand::RngCore;

fn wrap(
    dest_pk: &[u8; KEY_LEN],
    capability: [u8; KEY_LEN],
    ttl: TtlBucket,
    type_: MessageType,
    body: Vec<u8>,
) -> nemo_wire::OuterEnvelope {
    let inner = InnerEnvelope {
        delivery_capability: capability,
        ttl_bucket: ttl,
        idempotency_token: {
            let mut t = [0u8; KEY_LEN];
            rand::rngs::OsRng.fill_bytes(&mut t);
            t
        },
        padded_message: PaddedMessage::pad(type_, body).unwrap(),
    };
    seal_to_server(dest_pk, &inner).unwrap()
}

fn register(home: &mut HomeServer) -> (SigningKey, [u8; KEY_LEN]) {
    let sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let id = identity_id(&sk.verifying_key().to_bytes());
    home.register(id, sk.verifying_key().to_bytes()).unwrap();
    (sk, id)
}

fn auth(sk: &SigningKey, id: [u8; KEY_LEN], cursor: u64, limit: u64, ts: u64) -> MailboxOwnerAuth {
    MailboxOwnerAuth::sign(sk, id.to_vec(), cursor, limit, ts).unwrap()
}

fn ingest_outers(homes: &mut [&mut HomeServer], outers: Vec<nemo_wire::OuterEnvelope>) {
    for outer in outers {
        for home in homes.iter_mut() {
            if outer.destination_server_id == home.server_id() {
                home.ingest(&outer).unwrap();
            }
        }
    }
}

#[test]
fn share_token_burns_on_first_append() {
    let mut home = HomeServer::new();
    let (_sk, id) = register(&mut home);
    let token = home.mint_share_token(id, None).unwrap();
    let seq = home
        .ingest(&wrap(
            &home.hpke_public(),
            token,
            TtlBucket::DEFAULT,
            MessageType::DoubleRatchet,
            vec![1, 2, 3],
        ))
        .unwrap();
    assert_eq!(seq, 1);
    assert!(matches!(
        home.ingest(&wrap(
            &home.hpke_public(),
            token,
            TtlBucket::DEFAULT,
            MessageType::DoubleRatchet,
            vec![4],
        )),
        Err(ServerError::Denied)
    ));
}

#[test]
fn share_token_prekey_does_not_consume() {
    let mut home = HomeServer::new();
    let (_sk, id) = register(&mut home);
    home.publish_prekey(id, b"prekey-blob".to_vec()).unwrap();
    let token = home.mint_share_token(id, None).unwrap();
    assert_eq!(home.fetch_prekey(token).unwrap(), b"prekey-blob");
    assert_eq!(home.fetch_prekey(token).unwrap(), b"prekey-blob");
    home.ingest(&wrap(
        &home.hpke_public(),
        token,
        TtlBucket::DEFAULT,
        MessageType::DoubleRatchet,
        vec![1],
    ))
    .unwrap();
    assert!(matches!(home.fetch_prekey(token), Err(ServerError::Denied)));
}

#[test]
fn idempotent_append_same_seq() {
    let mut home = HomeServer::new();
    let (_sk, id) = register(&mut home);
    let cap = home.mint_contact_capability(id).unwrap();
    let outer = wrap(
        &home.hpke_public(),
        cap,
        TtlBucket::DEFAULT,
        MessageType::DoubleRatchet,
        vec![9],
    );
    assert_eq!(home.ingest(&outer).unwrap(), home.ingest(&outer).unwrap());
}

#[test]
fn owner_fetch_ack_not_capability() {
    let mut home = HomeServer::new();
    let (sk, id) = register(&mut home);
    let cap = home.mint_contact_capability(id).unwrap();
    home.ingest(&wrap(
        &home.hpke_public(),
        cap,
        TtlBucket::DEFAULT,
        MessageType::DoubleRatchet,
        vec![1],
    ))
    .unwrap();

    let ts = home.now;
    let stranger = SigningKey::generate(&mut rand::rngs::OsRng);
    let bad = auth(&stranger, id, 0, 16, ts);
    assert!(home.fetch(id, &bad, ts).is_err());

    let rows = home.fetch(id, &auth(&sk, id, 0, 16, ts), ts).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].seq, 1);

    let stale = auth(&sk, id, 0, 16, ts.saturating_sub(121));
    assert!(matches!(
        home.fetch(id, &stale, ts),
        Err(ServerError::OwnerAuth)
    ));

    home.ack(id, &auth(&sk, id, 1, 0, ts), ts).unwrap();
    assert!(home
        .fetch(id, &auth(&sk, id, 0, 16, ts), ts)
        .unwrap()
        .is_empty());
}

#[test]
fn contact_capability_rate_limit_is_generic_denied() {
    let mut home = HomeServer::new();
    let (_sk, id) = register(&mut home);
    let cap = home.mint_contact_capability(id).unwrap();
    for i in 0..nemo_server::home::RATE_PER_MIN {
        home.ingest(&wrap(
            &home.hpke_public(),
            cap,
            TtlBucket::DEFAULT,
            MessageType::DoubleRatchet,
            vec![i as u8],
        ))
        .unwrap();
    }
    assert!(matches!(
        home.ingest(&wrap(
            &home.hpke_public(),
            cap,
            TtlBucket::DEFAULT,
            MessageType::DoubleRatchet,
            vec![0xff],
        )),
        Err(ServerError::Denied)
    ));
}

#[test]
fn unknown_token_denied() {
    let mut home = HomeServer::new();
    let (_sk, _id) = register(&mut home);
    assert!(matches!(
        home.ingest(&wrap(
            &home.hpke_public(),
            [0x11; 32],
            TtlBucket::DEFAULT,
            MessageType::DoubleRatchet,
            vec![1],
        )),
        Err(ServerError::Denied)
    ));
}

#[test]
fn ttl_expires_before_fetch() {
    let mut home = HomeServer::new();
    let (sk, id) = register(&mut home);
    let cap = home.mint_contact_capability(id).unwrap();
    home.ingest(&wrap(
        &home.hpke_public(),
        cap,
        TtlBucket::SECONDS_60,
        MessageType::DoubleRatchet,
        vec![1],
    ))
    .unwrap();
    home.now += 61;
    assert!(home
        .fetch(id, &auth(&sk, id, 0, 16, home.now), home.now)
        .unwrap()
        .is_empty());
}

#[test]
fn retention_drops_oldest() {
    let mut home = HomeServer::with_limits(Limits {
        mailbox_max_bytes: 25_000,
        mailbox_max_age_secs: 14 * 24 * 3600,
    });
    let (sk, id) = register(&mut home);
    let cap = home.mint_contact_capability(id).unwrap();
    for i in 0..3u8 {
        home.ingest(&wrap(
            &home.hpke_public(),
            cap,
            TtlBucket::DEFAULT,
            MessageType::DoubleRatchet,
            vec![i],
        ))
        .unwrap();
    }
    let rows = home
        .fetch(id, &auth(&sk, id, 0, 16, home.now), home.now)
        .unwrap();
    assert!(rows.len() < 3);
    assert_eq!(rows.last().map(|r| r.seq), Some(3));
}

#[test]
fn group_fanout_and_remove() {
    let mut alice_home = HomeServer::new();
    let mut bob_home = HomeServer::new();
    let (alice_sk, alice_id) = register(&mut alice_home);
    let (bob_sk, bob_id) = register(&mut bob_home);
    let alice_cap = alice_home.mint_contact_capability(alice_id).unwrap();
    let bob_cap = bob_home.mint_contact_capability(bob_id).unwrap();

    let alice_group_sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let bob_group_sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let mut host = GroupHost::new();
    let created = host
        .create_group(
            alice_group_sk.verifying_key().to_bytes(),
            FanoutTarget {
                delivery_capability: alice_cap,
                home_hpke_public: alice_home.hpke_public(),
            },
        )
        .unwrap();
    let invite = GroupInvite::sign(
        &alice_group_sk,
        created.group_id.0,
        [0x22u8; 32],
        INTRO_TTL_30_MIN,
        None,
    )
    .unwrap();
    host.store_invite(created.group_id, &created.cred, &invite)
        .unwrap();
    let pending = host.accept(created.group_id, invite.nonce).unwrap();
    let admit = GroupAdmit::sign(&alice_group_sk, created.group_id.0, pending.pending_id).unwrap();
    host.admit(
        created.group_id,
        &created.cred,
        &admit,
        bob_group_sk.verifying_key().to_bytes(),
        FanoutTarget {
            delivery_capability: bob_cap,
            home_hpke_public: bob_home.hpke_public(),
        },
    )
    .unwrap();

    let appended = host
        .append(
            created.group_id,
            &created.cred,
            MessageType::MlsHandshake,
            vec![0xaa, 0xbb],
        )
        .unwrap();
    assert_eq!(appended.seq, 1);
    ingest_outers(&mut [&mut alice_home, &mut bob_home], appended.outers);

    let alice_rows = alice_home
        .fetch(
            alice_id,
            &auth(&alice_sk, alice_id, 0, 16, alice_home.now),
            alice_home.now,
        )
        .unwrap();
    let bob_rows = bob_home
        .fetch(
            bob_id,
            &auth(&bob_sk, bob_id, 0, 16, bob_home.now),
            bob_home.now,
        )
        .unwrap();
    assert_eq!(alice_rows.len(), 1);
    assert_eq!(bob_rows.len(), 1);
    assert_eq!(
        alice_rows[0].inner.padded_message.type_,
        MessageType::MlsHandshake
    );

    let bundle =
        RemoveBundle::sign(&alice_group_sk, pending.cred.credential_id, vec![1, 2, 3]).unwrap();
    let removed = host
        .append(
            created.group_id,
            &created.cred,
            MessageType::RemoveBundle,
            bundle.encode().unwrap(),
        )
        .unwrap();
    ingest_outers(&mut [&mut alice_home, &mut bob_home], removed.outers);
    assert!(matches!(
        host.append(
            created.group_id,
            &pending.cred,
            MessageType::MlsHandshake,
            vec![1],
        ),
        Err(ServerError::Denied)
    ));
    let bob_after = bob_home
        .fetch(
            bob_id,
            &auth(&bob_sk, bob_id, 0, 16, bob_home.now),
            bob_home.now,
        )
        .unwrap();
    assert!(bob_after
        .iter()
        .any(|r| r.inner.padded_message.type_ == MessageType::RemoveBundle));
}

#[test]
fn card_share_token_registers_and_bad_ttl_is_denied() {
    let mut home = HomeServer::new();
    let (_sk, id) = register(&mut home);
    let token = [0xABu8; 32];
    home.register_share_token(id, token, None).unwrap();
    assert_eq!(
        home.ingest(&wrap(
            &home.hpke_public(),
            token,
            TtlBucket::DEFAULT,
            MessageType::DoubleRatchet,
            vec![1],
        ))
        .unwrap(),
        1
    );
    assert!(home
        .register_share_token(id, [0xCDu8; 32], Some(99))
        .is_err());
}

#[test]
fn hpke_enc_replay_returns_original_seq() {
    let mut home = HomeServer::new();
    let (_sk, id) = register(&mut home);
    let token = home.mint_share_token(id, None).unwrap();
    let outer = wrap(
        &home.hpke_public(),
        token,
        TtlBucket::DEFAULT,
        MessageType::DoubleRatchet,
        vec![1, 2, 3],
    );
    let a = home.ingest(&outer).unwrap();
    let b = home.ingest(&outer).unwrap();
    assert_eq!(a, b);
    assert_eq!(a, 1);
}

#[test]
fn forged_invite_is_rejected() {
    let mut host = GroupHost::new();
    let alice_sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let stranger = SigningKey::generate(&mut rand::rngs::OsRng);
    let created = host
        .create_group(
            alice_sk.verifying_key().to_bytes(),
            FanoutTarget {
                delivery_capability: [1u8; 32],
                home_hpke_public: [2u8; 32],
            },
        )
        .unwrap();
    let forged = GroupInvite::sign(
        &stranger,
        created.group_id.0,
        [3u8; 32],
        INTRO_TTL_30_MIN,
        None,
    )
    .unwrap();
    assert!(matches!(
        host.store_invite(created.group_id, &created.cred, &forged),
        Err(ServerError::Denied)
    ));
}

#[test]
fn pending_join_expires_with_invite() {
    let mut host = GroupHost::new();
    let sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let created = host
        .create_group(
            sk.verifying_key().to_bytes(),
            FanoutTarget {
                delivery_capability: [1u8; 32],
                home_hpke_public: [2u8; 32],
            },
        )
        .unwrap();
    let invite =
        GroupInvite::sign(&sk, created.group_id.0, [4u8; 32], INTRO_TTL_30_MIN, None).unwrap();
    host.store_invite(created.group_id, &created.cred, &invite)
        .unwrap();
    let pending = host.accept(created.group_id, invite.nonce).unwrap();
    host.now += INTRO_TTL_30_MIN + 1;
    let admit = GroupAdmit::sign(&sk, created.group_id.0, pending.pending_id).unwrap();
    assert!(matches!(
        host.admit(
            created.group_id,
            &created.cred,
            &admit,
            [9u8; 32],
            FanoutTarget {
                delivery_capability: [3u8; 32],
                home_hpke_public: [4u8; 32],
            },
        ),
        Err(ServerError::Denied)
    ));
}

#[test]
fn group_file_reserve_upload_fetch() {
    let mut host = GroupHost::new();
    let sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let created = host
        .create_group(
            sk.verifying_key().to_bytes(),
            FanoutTarget {
                delivery_capability: [1u8; 32],
                home_hpke_public: [2u8; 32],
            },
        )
        .unwrap();
    let token = [0x11u8; 32];
    let reserve = AttachmentReserve {
        fetch_token: token,
        size_bucket: AttachmentSizeBucket::A1,
        ttl_bucket: TtlBucket::DAY,
    };
    host.append(
        created.group_id,
        &created.cred,
        MessageType::AttachmentReserve,
        reserve.encode(),
    )
    .unwrap();
    let body = vec![7u8; AttachmentSizeBucket::A1.inner_len()];
    host.upload_file(created.group_id, &created.cred, token, body.clone())
        .unwrap();
    assert_eq!(
        host.fetch_file(created.group_id, &created.cred, token)
            .unwrap(),
        body
    );
    assert!(host
        .upload_file(created.group_id, &created.cred, token, body)
        .is_err());
}

#[test]
fn group_file_budget_is_separate_from_stream() {
    let mut host = GroupHost::new();
    host.file_budget = 8;
    let created = host
        .create_group(
            [9u8; 32],
            FanoutTarget {
                delivery_capability: [1u8; 32],
                home_hpke_public: [2u8; 32],
            },
        )
        .unwrap();
    let token = [0x22u8; 32];
    let reserve = AttachmentReserve {
        fetch_token: token,
        size_bucket: AttachmentSizeBucket::A1,
        ttl_bucket: TtlBucket::DAY,
    };
    let err = host
        .append(
            created.group_id,
            &created.cred,
            MessageType::AttachmentReserve,
            reserve.encode(),
        )
        .unwrap_err();
    assert!(matches!(err, ServerError::Denied));
}

fn signed_card(home: &HomeServer, sk: &SigningKey, rev_pk: [u8; 32], seq: u64) -> ContactCard {
    let binding = HomeServerBinding::sign(
        sk,
        HomeServerBinding {
            server_id: home.server_id(),
            server_hpke_public_key: home.hpke_public(),
            host: "local".into(),
            seq,
            expires_at: home.now + 86_400,
            signature: [0; 64],
        },
    )
    .unwrap();
    ContactCard {
        identity_public_key: sk.verifying_key().to_bytes(),
        revocation_public_key: rev_pk,
        share_token: {
            let mut t = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut t);
            t
        },
        binding,
    }
}

#[test]
fn discovery_register_and_fetch() {
    let mut home = HomeServer::new();
    let sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let rev = SigningKey::generate(&mut rand::rngs::OsRng);
    let card = signed_card(&home, &sk, rev.verifying_key().to_bytes(), 1);
    let id = home.register_from_card(&card).unwrap();
    let row = home.discovery(id).unwrap();
    assert_eq!(row.identity_public_key, card.identity_public_key);
    assert_eq!(row.binding.seq, 1);
    assert!(row.revocation.is_none());
    home.publish_prekey(id, vec![1, 2, 3]).unwrap();
    assert_eq!(home.fetch_prekey(card.share_token).unwrap(), vec![1, 2, 3]);
}

#[test]
fn enqueue_from_owner_requires_auth() {
    let mut home = HomeServer::new();
    let (sk, id) = register(&mut home);
    let cap = home.mint_contact_capability(id).unwrap();
    let outer = wrap(
        &home.hpke_public(),
        cap,
        TtlBucket::DEFAULT,
        MessageType::DoubleRatchet,
        vec![1],
    );
    let bad = auth(&sk, id, 0, 16, home.now - 10_000);
    assert!(matches!(
        home.enqueue_from_owner(id, &bad, home.now, outer.clone()),
        Err(ServerError::OwnerAuth)
    ));
    let ok = auth(&sk, id, 0, 16, home.now);
    assert!(matches!(
        home.enqueue_from_owner(id, &ok, home.now, outer).unwrap(),
        Enqueue::Local(1)
    ));
}

#[test]
fn higher_binding_seq_disables_old_mailbox() {
    let mut old = HomeServer::new();
    let new = HomeServer::new();
    let sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let rev = SigningKey::generate(&mut rand::rngs::OsRng);
    let rev_pk = rev.verifying_key().to_bytes();
    let card = signed_card(&old, &sk, rev_pk, 1);
    let id = old.register_from_card(&card).unwrap();
    let cap = old.mint_contact_capability(id).unwrap();
    old.ingest(&wrap(
        &old.hpke_public(),
        cap,
        TtlBucket::DEFAULT,
        MessageType::DoubleRatchet,
        vec![1],
    ))
    .unwrap();

    let migrated = HomeServerBinding::sign(
        &sk,
        HomeServerBinding {
            server_id: new.server_id(),
            server_hpke_public_key: new.hpke_public(),
            host: "other.example".into(),
            seq: 2,
            expires_at: old.now + 86_400,
            signature: [0; 64],
        },
    )
    .unwrap();
    old.observe_binding(id, &migrated).unwrap();
    assert!(old.mailbox_disabled(id));
    assert!(old.mint_contact_capability(id).is_err());
    let row = old.discovery(id).unwrap();
    assert_eq!(row.binding.seq, 2);
}

#[test]
fn revocation_wipes_and_hosts_append() {
    let mut home = HomeServer::new();
    let mut host = GroupHost::new();
    let sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let rev = SigningKey::generate(&mut rand::rngs::OsRng);
    let card = signed_card(&home, &sk, rev.verifying_key().to_bytes(), 1);
    let id = home.register_from_card(&card).unwrap();
    let group_sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let created = host
        .create_group(
            group_sk.verifying_key().to_bytes(),
            FanoutTarget {
                delivery_capability: [1u8; 32],
                home_hpke_public: home.hpke_public(),
            },
        )
        .unwrap();
    home.note_group_membership(id, created.group_id);
    let stmt = RevocationStatement::sign(&rev, id, home.now).unwrap();
    let groups = home.ingest_revocation(&stmt).unwrap();
    assert_eq!(groups, vec![created.group_id]);
    assert!(home.mailbox_disabled(id));
    let out = host
        .append_host_revocation(created.group_id, &stmt)
        .unwrap();
    assert_eq!(out.seq, 1);
    assert_eq!(
        home.discovery(id).unwrap().revocation.unwrap().identity_id,
        id
    );
}
