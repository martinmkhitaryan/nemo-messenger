-- Phase 8 persistence. Columns are only fields required by phases 1–5
-- (ADR-0027). received_at / expires_at are internal (ADR-0013) and MUST NOT
-- be returned on client HTTP. No plaintext, nicknames, senders, or receipts.

CREATE TABLE server_identity (
    singleton             BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    hpke_secret           BYTEA NOT NULL,
    hpke_public           BYTEA NOT NULL,
    sign_secret           BYTEA NOT NULL,
    sign_public           BYTEA NOT NULL,
    host                  TEXT NOT NULL,
    s2s_port              INTEGER NOT NULL
);

CREATE TABLE identities (
    identity_id           BYTEA PRIMARY KEY,
    identity_public_key   BYTEA NOT NULL,
    revocation_public_key BYTEA NOT NULL,
    binding_cbor          BYTEA NOT NULL,
    binding_seq           BIGINT NOT NULL,
    revocation_cbor       BYTEA,
    mailbox_disabled      BOOLEAN NOT NULL DEFAULT FALSE,
    mailbox_next_seq      BIGINT NOT NULL DEFAULT 1,
    mailbox_bytes         BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE mailbox_rows (
    identity_id           BYTEA NOT NULL REFERENCES identities (identity_id),
    seq                   BIGINT NOT NULL,
    received_at           BIGINT NOT NULL,
    expires_at            BIGINT NOT NULL,
    inner_padded          BYTEA NOT NULL,
    idempotency_token     BYTEA NOT NULL,
    PRIMARY KEY (identity_id, seq)
);

CREATE UNIQUE INDEX mailbox_idempotency
    ON mailbox_rows (identity_id, idempotency_token);

CREATE TABLE tokens (
    token                 BYTEA PRIMARY KEY,
    kind                  SMALLINT NOT NULL, -- 1 share, 2 contact
    mailbox               BYTEA NOT NULL REFERENCES identities (identity_id),
    expires_at            BIGINT,
    burned                BOOLEAN NOT NULL DEFAULT FALSE,
    reserved_prekey       BYTEA,
    grace_until           BIGINT,
    window_start          BIGINT,
    window_count          INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE prekeys (
    identity_id           BYTEA NOT NULL REFERENCES identities (identity_id),
    pos                   BIGSERIAL,
    blob                  BYTEA NOT NULL,
    PRIMARY KEY (identity_id, pos)
);

CREATE TABLE groups (
    group_id              BYTEA PRIMARY KEY,
    next_seq              BIGINT NOT NULL DEFAULT 1,
    bytes                 BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE group_creds (
    group_id              BYTEA NOT NULL REFERENCES groups (group_id),
    credential_id         BYTEA NOT NULL,
    credential_secret     BYTEA NOT NULL,
    live                  BOOLEAN NOT NULL,
    signing_pk            BYTEA,
    fanout_capability     BYTEA,
    fanout_hpke           BYTEA,
    PRIMARY KEY (group_id, credential_id)
);

CREATE TABLE group_invites (
    group_id              BYTEA NOT NULL REFERENCES groups (group_id),
    nonce                 BYTEA NOT NULL,
    expires_at            BIGINT NOT NULL,
    invitee_binding       BYTEA,
    PRIMARY KEY (group_id, nonce)
);

CREATE TABLE group_pending (
    group_id              BYTEA NOT NULL REFERENCES groups (group_id),
    pending_id            BYTEA NOT NULL,
    credential_id         BYTEA NOT NULL,
    credential_secret     BYTEA NOT NULL,
    expires_at            BIGINT NOT NULL,
    PRIMARY KEY (group_id, pending_id)
);

CREATE TABLE group_stream (
    group_id              BYTEA NOT NULL REFERENCES groups (group_id),
    seq                   BIGINT NOT NULL,
    type                  SMALLINT NOT NULL,
    body                  BYTEA NOT NULL,
    received_at           BIGINT NOT NULL,
    PRIMARY KEY (group_id, seq)
);

CREATE TABLE group_files (
    fetch_token           BYTEA PRIMARY KEY,
    group_id              BYTEA NOT NULL REFERENCES groups (group_id),
    owner_cred            BYTEA NOT NULL,
    size_bytes            INTEGER NOT NULL,
    expires_at            BIGINT NOT NULL,
    bytes                 BYTEA
);

CREATE TABLE memberships (
    identity_id           BYTEA NOT NULL REFERENCES identities (identity_id),
    group_id              BYTEA NOT NULL,
    PRIMARY KEY (identity_id, group_id)
);

CREATE TABLE peers (
    server_id             BYTEA PRIMARY KEY,
    bundle_cbor           BYTEA NOT NULL,
    refused               BOOLEAN NOT NULL DEFAULT FALSE,
    last_recv_counter     BIGINT NOT NULL DEFAULT 0,
    last_send_counter     BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE outbound (
    id                    BIGSERIAL PRIMARY KEY,
    dest                  BYTEA NOT NULL,
    outer_cbor            BYTEA NOT NULL,
    enqueued_at           BIGINT NOT NULL,
    next_attempt          BIGINT NOT NULL,
    backoff_secs          BIGINT NOT NULL
);

CREATE TABLE hpke_seen (
    enc                   BYTEA PRIMARY KEY,
    seq                   BIGINT NOT NULL,
    seen_at               BIGINT NOT NULL
);
