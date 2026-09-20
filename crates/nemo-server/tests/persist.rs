//! Live Postgres round-trip. Skipped unless `NEMO_TEST_DATABASE_URL` is set
//! so `cargo test` never truncates an operator database.

use nemo_server::group::{FanoutTarget, GroupHost};
use nemo_server::home::HomeServer;
use nemo_server::pg;
use nemo_wire::envelope::{InnerEnvelope, MessageType, PaddedMessage, TtlBucket};
use nemo_wire::hpke::seal_to_server;
use nemo_wire::ids::{identity_id, KEY_LEN};
use nemo_wire::SigningKey;
use rand::RngCore;

fn env_url() -> Option<String> {
    std::env::var("NEMO_TEST_DATABASE_URL").ok()
}

#[tokio::test]
async fn postgres_survives_reload() {
    let Some(url) = env_url() else {
        return;
    };
    let pool = pg::connect(&url).await.expect("connect");
    let mut home = HomeServer::advertise("pg-test".to_string(), 8443);
    let mut groups = GroupHost::new();
    let sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let pk = sk.verifying_key().to_bytes();
    let id = identity_id(&pk);
    home.register(id, pk).unwrap();
    home.publish_prekey(id, vec![9, 8, 7]).unwrap();
    let share = home.mint_share_token(id, None).unwrap();
    assert_eq!(home.fetch_prekey(share).unwrap(), vec![9, 8, 7]);
    let contact = home.mint_contact_capability(id).unwrap();
    let inner = InnerEnvelope {
        delivery_capability: contact,
        ttl_bucket: TtlBucket::DEFAULT,
        idempotency_token: {
            let mut t = [0u8; KEY_LEN];
            rand::rngs::OsRng.fill_bytes(&mut t);
            t
        },
        padded_message: PaddedMessage::pad(MessageType::DoubleRatchet, vec![1, 2, 3]).unwrap(),
    };
    let outer = seal_to_server(&home.hpke_public(), &inner).unwrap();
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

    pg::flush(&pool, &home, &groups).await.expect("flush");
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
