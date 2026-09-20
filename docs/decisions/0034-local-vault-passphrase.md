# ADR-0034: Local vault passphrase, Argon2id, and SQLCipher

- **Status:** Accepted
- **Date:** 2026-09-20
- **Affects:** README.md sections 0, 51, 53.11; ADR-0004 (mnemonic never stored); ADR-0023 (vault is not backup); ADR-0028 (SQLCipher via `rusqlite` in `nemo-core`)

## Context

[ADR-0028](0028-implementation-languages-and-libraries.md) puts the local vault in `nemo-core` as SQLCipher through `rusqlite`. The Compose shell must not persist ratchet or MLS keys. [ADR-0004](0004-revocation-key.md) forbids writing the revocation secret after the one-time export. [ADR-0023](0023-no-backup-no-recovery.md) forbids exporting cryptographic state off the device.

Unlock was unspecified. An installation that exists only in RAM is lost on process restart, so 1:1 sessions, prekeys, and the identity key cannot ship. The remaining choice is how the user unlocks the on-disk store: OS keystore, an empty/default passphrase, SQLCipher's built-in PBKDF2, or an application passphrase with a modern KDF.

v1 platforms are Android, Linux, and Windows (ADR-0026). A design that requires Android Keystore or the Windows DPAPI would split the unlock model.

## Decision

- **Unlock:** a user-chosen **application passphrase**. v1 does not require an OS keystore, TPM, or Secure Enclave. Optional platform wrapping may be added later without changing the on-disk format.
- **Minimum length:** 8 Unicode scalar values. Empty and shorter passphrases are refused.
- **KDF:** Argon2id, RFC 9106. Parameters are stored **unencrypted** next to the ciphertext so they can be raised later:

  | Parameter | v1 value |
  | --- | --- |
  | Memory | 65536 KiB (64 MiB) |
  | Iterations | 3 |
  | Parallelism | 1 |
  | Salt | 16 random bytes |
  | Output | 32 bytes |

- **Cipher:** SQLCipher (SQLCipher 4 defaults) via `rusqlite`. The Argon2id output is supplied as a **raw 256-bit key** (`PRAGMA key = "x'<64 hex>'"`). SQLCipher's own PBKDF2 is not used.
- **Layout:** a vault is a directory:

  ```text
  <dir>/kdf.cbor     unencrypted {version, salt, m_cost, t_cost, p_cost}
  <dir>/store.db     SQLCipher database
  ```

  `kdf.cbor` is canonical CBOR. `store.db` holds an opaque installation snapshot (identity secret, revocation **public** key, libsignal store including sessions and PQXDH one-time / used Kyber markers, prekey counters, home-binding `seq`). MLS group state is **not** in this snapshot yet.
- **Never stored:** the revocation mnemonic or revocation private key (ADR-0004). Lost passphrase loses the installation; that is the same cost as losing the device (ADR-0003, ADR-0023). The vault is not a backup and is not copied to the home server.
- **Wrong passphrase:** open fails closed. The client MUST NOT distinguish "wrong passphrase" from "corrupt file" in UX copy beyond a single unlock-failed state.
- **Compose / UniFFI:** the shell passes the directory path and passphrase across FFI. It MUST NOT write identity, ratchet, or MLS keys of its own.

## Pros

- One unlock model on Android, Linux, and Windows.
- Argon2id is the current password-hashing baseline; SQLCipher is the library ADR-0028 already named.
- Raw-key SQLCipher keeps KDF policy in this project, not in SQLCipher's default iter count.
- Matches "device is the only place plaintext exists" (ADR-0023): ciphertext at rest is still on that device.

## Cons

- A forgotten passphrase is a dead identity. Users who wanted "PIN plus phone backup" will not get it.
- 64 MiB Argon2id is noticeable on low-end Android and at every unlock.
- Two files; copying `store.db` without `kdf.cbor` makes the vault unopenable.
- No OS-keystore convenience (biometric unlock, no passphrase-in-memory while the app is foreground) in v1.

## Alternatives considered

### SQLCipher passphrase mode (built-in PBKDF2)

Rejected: weaker than Argon2id at comparable UX cost; iter count would be a silent SQLCipher default.

### Android Keystore / DPAPI / libsecret wrapping the SQLCipher key

Deferred: three platform paths, no Linux/Windows biometric story that matches Android, and a lost-keystore-blob failure mode. May wrap the same raw key later without a format break.

### Single encrypted blob (ChaCha20-Poly1305) instead of SQLCipher

Rejected: ADR-0028 already bound SQLCipher. A second AEAD would be extra cryptography for the same job.

### Empty or machine-generated passphrase

Rejected: an unlocked file on disk is not a vault. A generated secret still needs somewhere to live (keystore), which is the deferred alternative above.

### Store the revocation mnemonic in the vault

Rejected by ADR-0004: the phrase exists to kill a stolen device from outside it.

## Consequences

- `nemo-core` grows a vault module. `nemo-server` stays unaware of it.
- Identity creation UX: show the revocation phrase **and** choose a vault passphrase; state that both are unrecoverable.
- MLS groups, contact nicknames, and message display rows may share this SQLCipher file in later slices; they are not in the v1 identity snapshot.
- Raising Argon2 parameters is a `kdf.cbor` version bump plus a re-encrypt; old files keep their stored parameters.

## History

- 2026-09-20 — Accepted. Application passphrase, Argon2id, SQLCipher raw key, mnemonic never stored.
