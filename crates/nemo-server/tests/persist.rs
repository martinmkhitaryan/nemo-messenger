//! Live Postgres round-trip for incremental persist. Skipped unless
//! `NEMO_TEST_DATABASE_URL` is set so `cargo test` never truncates an
//! operator database.
//!
//! Round-trips every dirty path: each test mutates, persists, reloads from
//! Postgres, and asserts the reloaded state behaves identically.

use nemo_server::group::{FanoutTarget, GroupHost};
use nemo_server::home::HomeServer;
use nemo_server::pg;
use nemo_wire::envelope::{InnerEnvelope, MessageType, PaddedMessage, TtlBucket};
use nemo_wire::hpke::seal_to_server;
use nemo_wire::ids::{identity_id, KEY_LEN};
use nemo_wire::{
    AttachmentReserve, AttachmentSizeBucket, ContactCard, GroupInvite, HomeServerBinding,
    MailboxOwnerAuth, OuterEnvelope, RevocationStatement, SigningKey,
};
use rand::Rng;

const NOW: u64 = 1_700_000_100;

fn env_url() -> Option<String> {
    std::env::var("NEMO_TEST_DATABASE_URL").ok()
}

fn seal_for(home: &HomeServer, cap: [u8; KEY_LEN], body: Vec<u8>) -> OuterEnvelope {
    let mut t = [0u8; KEY_LEN];
    rand::rng().fill_bytes(&mut t);
    let inner = InnerEnvelope {
        delivery_capability: cap,
        ttl_bucket: TtlBucket::DEFAULT,
        idempotency_token: t,
        padded_message: PaddedMessage::pad(MessageType::DoubleRatchet, body).unwrap(),
    };
    seal_to_server(&home.hpke_public(), &inner).unwrap()
}

fn owner_auth(sk: &SigningKey, id: &[u8; KEY_LEN], cursor: u64) -> MailboxOwnerAuth {
    MailboxOwnerAuth::sign(sk, id.to_vec(), cursor, 64, NOW).unwrap()
}

fn register_card(
    home: &mut HomeServer,
    sk: &SigningKey,
    rev_pk: [u8; KEY_LEN],
) -> (ContactCard, [u8; KEY_LEN]) {
    let pk = sk.verifying_key().to_bytes();
    let binding = HomeServerBinding::sign(
        sk,
        HomeServerBinding {
            server_id: home.server_id(),
            server_hpke_public_key: home.hpke_public(),
            host: "pg-test".into(),
            seq: 1,
            expires_at: NOW + 100_000,
            signature: [0; 64],
        },
    )
    .unwrap();
    let mut share = [0u8; KEY_LEN];
    rand::rng().fill_bytes(&mut share);
    let card = ContactCard {
        identity_public_key: pk,
        revocation_public_key: rev_pk,
        share_token: share,
        binding,
    };
    let id = home.register_from_card(&card).unwrap();
    assert_eq!(id, identity_id(&pk));
    (card, id)
}

async fn flow_survives_reload(pool: &sqlx::PgPool) {
    let mut home = HomeServer::advertise("pg-test".to_string(), 8443);
    let mut groups = GroupHost::new();
    let sk = SigningKey::generate(&mut rand::rng());
    let pk = sk.verifying_key().to_bytes();
    let id = identity_id(&pk);
    home.register(id, pk).unwrap();
    home.publish_prekey(id, vec![9, 8, 7]).unwrap();
    let share = home.mint_share_token(id, None).unwrap();
    assert_eq!(home.fetch_prekey(share).unwrap(), vec![9, 8, 7]);
    let contact = home.mint_contact_capability(id).unwrap();
    let outer = seal_for(&home, contact, vec![1, 2, 3]);
    home.ingest(&outer).unwrap();

    let created = groups
        .create_group(
            pk,
            FanoutTarget {
                delivery_capability: contact,
                home_hpke_public: home.hpke_public(),
            },
        )
        .unwrap();

    pg::persist(&pool, &mut home, &mut groups).await.expect("persist");
    let (mut loaded, mut loaded_groups) = pg::load(&pool).await.expect("load").expect("identity row");
    assert_eq!(loaded.server_id(), home.server_id());
    assert_eq!(loaded.bundle().host, "pg-test");
    assert_eq!(loaded.hpke_public(), home.hpke_public());
    assert!(!loaded.mailbox_disabled(id));
    assert_eq!(loaded.fetch_prekey(share).unwrap(), vec![9, 8, 7]);
    loaded_groups
        .append(
            created.group_id,
            &created.cred,
            MessageType::MlsHandshake,
            vec![1, 2, 3],
        )
        .expect("group cred restored");
}

async fn flow_incremental_rounds(pool: &sqlx::PgPool) {
    let mut home = HomeServer::advertise("pg-test-rounds".to_string(), 8443);
    let mut groups = GroupHost::new();

    let sk = SigningKey::generate(&mut rand::rng());
    let rev_sk = SigningKey::generate(&mut rand::rng());
    let rev_pk = rev_sk.verifying_key().to_bytes();
    let pk = sk.verifying_key().to_bytes();
    let (card, id) = register_card(&mut home, &sk, rev_pk);
    home.publish_prekey(id, vec![1]).unwrap();
    home.publish_prekey(id, vec![2]).unwrap();
    let contact = home.mint_contact_capability(id).unwrap();

    // Round 1: one mailbox row + prekey reservation + group with stream row.
    let outer1 = seal_for(&home, contact, vec![10]);
    let seq1 = home.ingest(&outer1).unwrap();
    assert_eq!(seq1, 1);
    assert_eq!(home.mailbox_head(id), 1);
    let fetched = home.fetch_prekey(card.share_token).unwrap();
    assert_eq!(fetched, vec![1]);
    // Reservation path: same blob without consuming again.
    assert_eq!(home.fetch_prekey(card.share_token).unwrap(), vec![1]);

    let created = groups
        .create_group(
            pk,
            FanoutTarget {
                delivery_capability: contact,
                home_hpke_public: home.hpke_public(),
            },
        )
        .unwrap();
    // Fanout target is live: the stream append also drops a mailbox row.
    let appended = groups
        .append(created.group_id, &created.cred, MessageType::MlsHandshake, vec![7, 7])
        .unwrap();
    assert_eq!(appended.seq, 1);
    for outer in appended.outers {
        home.enqueue(outer).unwrap();
    }
    assert_eq!(home.mailbox_head(id), 2);
    home.note_group_membership(id, created.group_id);

    // Invite lifecycle + fanout refresh: covers invites/pending/cred writes.
    let mut nonce = [0u8; KEY_LEN];
    rand::rng().fill_bytes(&mut nonce);
    let invite = GroupInvite::sign(&sk, created.group_id.0, nonce, 1800, None).unwrap();
    groups
        .store_invite(created.group_id, &created.cred, &invite)
        .unwrap();
    let pending = groups.accept(created.group_id, nonce).unwrap();
    assert_eq!(pending.cred.credential_id == created.cred.credential_id, false);
    groups
        .refresh_fanout(
            created.group_id,
            &created.cred,
            FanoutTarget {
                delivery_capability: contact,
                home_hpke_public: home.hpke_public(),
            },
        )
        .unwrap();

    // Federation peer row round-trips too.
    let mut home2 = HomeServer::advertise("pg-test-peer".to_string(), 9443);
    nemo_server::pin_each_other(&mut home, &mut home2).unwrap();
    nemo_server::refuse(&mut home, home2.server_id());

    pg::persist(&pool, &mut home, &mut groups).await.expect("persist r1");
    let (mut home, mut groups) = pg::load(&pool).await.expect("load r1").expect("state r1");

    assert_eq!(home.mailbox_head(id), 2);
    assert!(home.bundle_pin(home2.server_id()).is_some());
    assert!(home.peer_refused(home2.server_id()));
    // Dedup window survived: same outer replays to the same seq, no new row.
    assert_eq!(home.ingest(&outer1).unwrap(), seq1);
    assert_eq!(home.mailbox_head(id), 2);
    // Reserved prekey + consumed queue survived.
    assert_eq!(home.fetch_prekey(card.share_token).unwrap(), vec![1]);
    // Contact capability + rate window survived: second message works.
    let outer2 = seal_for(&home, contact, vec![11]);
    assert_eq!(home.ingest(&outer2).unwrap(), 3);
    // Group cred + stream survived.
    let appended2 = groups
        .append(created.group_id, &created.cred, MessageType::MlsHandshake, vec![8])
        .unwrap();
    assert_eq!(appended2.seq, 2);

    // Round 2: ack the first rows, reserve + upload a file, then persist.
    home.ack(id, &owner_auth(&sk, &id, 2), NOW).unwrap();
    assert_eq!(home.mailbox_head(id), 3);
    let reserve = AttachmentReserve {
        fetch_token: {
            let mut t = [0u8; KEY_LEN];
            rand::rng().fill_bytes(&mut t);
            t
        },
        size_bucket: AttachmentSizeBucket::A1,
        ttl_bucket: TtlBucket::DEFAULT,
    };
    let token = reserve.fetch_token;
    groups
        .append(
            created.group_id,
            &created.cred,
            MessageType::AttachmentReserve,
            reserve.encode(),
        )
        .unwrap();
    groups
        .upload_file(created.group_id, &created.cred, token, vec![5u8; 262_144])
        .unwrap();
    pg::persist(&pool, &mut home, &mut groups).await.expect("persist r2");
    let (mut home, mut groups) = pg::load(&pool).await.expect("load r2").expect("state r2");

    // Acked rows are gone, the live row and stream/file state remain.
    assert_eq!(home.mailbox_head(id), 3);
    let rows = home
        .fetch(id, &owner_auth(&sk, &id, 2), NOW)
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].seq, 3);
    assert_eq!(
        groups
            .fetch_file(created.group_id, &created.cred, token)
            .unwrap()
            .len(),
        262_144
    );

    // Round 3: revocation wipes the mailbox, keys and memberships.
    let stmt = RevocationStatement::sign(&rev_sk, id, NOW).unwrap();
    let removed = home.ingest_revocation(&stmt).unwrap();
    assert_eq!(removed, vec![created.group_id]);
    assert!(home.mailbox_disabled(id));
    pg::persist(&pool, &mut home, &mut groups).await.expect("persist r3");
    let (mut home, _) = pg::load(&pool).await.expect("load r3").expect("state r3");
    assert!(home.mailbox_disabled(id));
    assert!(home.fetch(id, &owner_auth(&sk, &id, 3), NOW).is_err());
    assert!(home.fetch_prekey(card.share_token).is_err());
}

async fn flow_clean_persist_is_noop(pool: &sqlx::PgPool) {
    let mut home = HomeServer::advertise("pg-test-clean".to_string(), 8443);
    let mut groups = GroupHost::new();
    // Fresh advertise only dirties server_identity: one row, then clean.
    pg::persist(&pool, &mut home, &mut groups).await.expect("persist");
    let before = pg::load(&pool).await.expect("load").expect("state");
    assert_eq!(before.0.bundle().host, "pg-test-clean");
    // Second persist with no changes must succeed trivially.
    pg::persist(&pool, &mut home, &mut groups).await.expect("noop persist");
}

async fn flow_queues_and_windows(pool: &sqlx::PgPool) {
    let mut home = HomeServer::advertise("pg-test-queues".to_string(), 8443);
    let mut groups = GroupHost::new();
    let sk = SigningKey::generate(&mut rand::rng());
    let pk = sk.verifying_key().to_bytes();
    let id = identity_id(&pk);
    home.register(id, pk).unwrap();

    // Prekey FIFO order survives a queue rewrite.
    home.publish_prekey(id, vec![10]).unwrap();
    home.publish_prekey(id, vec![20]).unwrap();
    home.publish_prekey(id, vec![30]).unwrap();
    let s1 = home.mint_share_token(id, None).unwrap();
    let s2 = home.mint_share_token(id, None).unwrap();
    let s3 = home.mint_share_token(id, None).unwrap();
    let contact = home.mint_contact_capability(id).unwrap();
    pg::persist(&pool, &mut home, &mut groups).await.expect("persist q1");
    let (mut home, mut groups) = pg::load(&pool).await.expect("load q1").expect("state q1");
    assert_eq!(home.fetch_prekey(s1).unwrap(), vec![10]);
    assert_eq!(home.fetch_prekey(s2).unwrap(), vec![20]);
    assert_eq!(home.fetch_prekey(s3).unwrap(), vec![30]);

    // Remote-destined envelope sits in the outbound queue across reloads.
    let mut dest = [0u8; KEY_LEN];
    rand::rng().fill_bytes(&mut dest);
    let inner = InnerEnvelope {
        delivery_capability: contact,
        ttl_bucket: TtlBucket::DEFAULT,
        idempotency_token: {
            let mut t = [0u8; KEY_LEN];
            rand::rng().fill_bytes(&mut t);
            t
        },
        padded_message: PaddedMessage::pad(MessageType::DoubleRatchet, vec![9]).unwrap(),
    };
    let remote = nemo_wire::envelope::OuterEnvelope {
        destination_server_id: dest,
        hpke_ciphertext: seal_for(&home, contact, vec![9]).hpke_ciphertext,
    };
    let _ = inner;
    let outbound_before = home.outbound_len();
    assert!(matches!(
        home.enqueue(remote).unwrap(),
        nemo_server::federation::Enqueue::Queued
    ));
    assert_eq!(home.outbound_len(), outbound_before + 1);
    pg::persist(&pool, &mut home, &mut groups).await.expect("persist q2");
    let (mut home, mut groups) = pg::load(&pool).await.expect("load q2").expect("state q2");
    assert_eq!(home.outbound_len(), outbound_before + 1);

    // Fill the HPKE dedup window past its 1024-entry wrap, then reload.
    // (30 ingests/min per capability: rotate through fresh caps.)
    let mut caps = vec![contact];
    for _ in 0..34 {
        caps.push(home.mint_contact_capability(id).unwrap());
    }
    for i in 0..1030u64 {
        let outer = seal_for(&home, caps[(i % 35) as usize], vec![(i % 251) as u8]);
        home.ingest(&outer).unwrap();
    }
    assert_eq!(home.mailbox_head(id), 1030);
    pg::persist(&pool, &mut home, &mut groups).await.expect("persist q3");
    let (home, _) = pg::load(&pool).await.expect("load q3").expect("state q3");
    assert_eq!(home.mailbox_head(id), 1030);
}

async fn flow_token_gc(pool: &sqlx::PgPool) {
    use nemo_server::home::CONTACT_CAP_IDLE_SECS;
    let mut home = HomeServer::advertise("pg-test-gc".to_string(), 8443);
    let mut groups = GroupHost::new();
    let base: u64 = 1_700_000_100;
    home.now = base;

    let sk = SigningKey::generate(&mut rand::rng());
    let pk = sk.verifying_key().to_bytes();
    let id = identity_id(&pk);
    home.register(id, pk).unwrap();
    home.publish_prekey(id, vec![42]).unwrap();
    home.publish_prekey(id, vec![43]).unwrap();

    // Shares: plain-expired, burned bare, burned+reserved.
    let s_expire = home.mint_share_token(id, None).unwrap();
    let s_burn = home.mint_share_token(id, None).unwrap();
    let s_res = home.mint_share_token(id, None).unwrap();
    assert_eq!(home.fetch_prekey(s_res).unwrap(), vec![42]);
    home.ingest(&seal_for(&home, s_res, vec![1])).unwrap();
    home.ingest(&seal_for(&home, s_burn, vec![2])).unwrap();

    // Contacts: two old idle, one newest.
    let c_old1 = home.mint_contact_capability(id).unwrap();
    home.now = base + 24 * 3600;
    let c_old2 = home.mint_contact_capability(id).unwrap();
    home.now = base + 2 * 24 * 3600;
    let c_new = home.mint_contact_capability(id).unwrap();
    home.ingest(&seal_for(&home, c_new, vec![3])).unwrap();

    pg::persist(pool, &mut home, &mut groups)
        .await
        .expect("persist gc r1");
    let count_r1: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tokens")
        .fetch_one(pool)
        .await
        .expect("count r1");
    assert!(count_r1 >= 6, "expected tokens persisted, got {count_r1}");

    // Past the 30d contact fuse (and well past all share fuses: 1d/7d).
    home.now = base + 2 * 24 * 3600 + CONTACT_CAP_IDLE_SECS + 100;
    // Refresh newest use so it is not idle.
    home.ingest(&seal_for(&home, c_new, vec![4])).unwrap();
    let swept = home.sweep_dead_tokens();
    assert!(swept >= 4, "expected share+contact sweep, got {swept}");

    pg::persist(pool, &mut home, &mut groups)
        .await
        .expect("persist gc r2");
    let (mut loaded, mut loaded_groups) =
        pg::load(pool).await.expect("load gc").expect("state gc");
    let count_r2: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tokens")
        .fetch_one(pool)
        .await
        .expect("count r2");
    assert!(
        count_r2 < count_r1,
        "swept rows must stay gone after reload ({count_r1} -> {count_r2})"
    );
    // Newest contact cap still sends.
    loaded.now = home.now;
    let seq = loaded.ingest(&seal_for(&loaded, c_new, vec![5])).unwrap();
    assert!(seq > 0);
    // Old contact caps are gone: unknown-token Denied.
    assert!(loaded.ingest(&seal_for(&loaded, c_old1, vec![6])).is_err());
    assert!(loaded.ingest(&seal_for(&loaded, c_old2, vec![7])).is_err());
    // Swept shares stay gone.
    assert!(loaded.fetch_prekey(s_expire).is_err());
    assert!(loaded.fetch_prekey(s_burn).is_err());
    assert!(loaded.fetch_prekey(s_res).is_err());
    let count_r3: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tokens")
        .fetch_one(pool)
        .await
        .expect("count r3");
    assert_eq!(
        count_r2, count_r3,
        "failed sends must not resurrect swept rows"
    );

    pg::persist(pool, &mut loaded, &mut loaded_groups)
        .await
        .expect("persist gc r3");
}

#[tokio::test]
async fn postgres_incremental_persist() {
    // One driver, sequential flows: all flows share a single database, so
    // they must not race each other (notably on the server_identity row).
    let Some(url) = env_url() else {
        return;
    };
    let pool = pg::connect(&url).await.expect("connect");
    flow_clean_persist_is_noop(&pool).await;
    flow_survives_reload(&pool).await;
    flow_incremental_rounds(&pool).await;
    flow_queues_and_windows(&pool).await;
    flow_token_gc(&pool).await;
}
