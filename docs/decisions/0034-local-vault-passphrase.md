# ADR-0034: Local vault passphrase, Argon2id, and SQLCipher

- **Status:** Accepted
- **Date:** 2026-09-20
- **Affects:** README.md sections 0, 51, 53.11; ADR-0004 (mnemonic never stored); ADR-0023 (vault is not backup); ADR-0028 (SQLCipher via `rusqlite` in `nemo-core`)

## Context

[ADR-0028](0028-implementation-languages-and-libraries.md) puts the local vault in `nemo-core` as SQLCipher through `rusqlite`. The Compose shell must not persist ratchet or MLS keys. [ADR-0004](0004-revocation-key.md) forbids writing the revocation secret after the one-time export. [ADR-0023](0023-no-backup-no-recovery.md) forbids exporting cryptographic state off the device.

Unlock was unspecified. An installation that exists only in RAM is lost on process restart, so 1:1 sessions, prekeys, and the identity key cannot ship. The remaining choice is how the user unlocks the on-disk store: OS keystore, an empty/default passphrase, SQLCipher's built-in PBKDF2, or an application passphrase with a modern KDF.

v1 platforms are Android, Linux, and Windows (ADR-0026). Passphrase-only wrapping lets anyone who copies `kdf.cbor` and `store.db` open them on another machine with the passphrase. Mixing in a device-held secret closes that copy-off-device path without changing the unlock UX.

## Decision

- **Unlock:** a user-chosen **application passphrase**.
- **Device bind:** new vaults (`kdf.cbor` version 2) mix the Argon2id output with a 32-byte device secret (`db_key = argon2id(passphrase) XOR device_secret`). The device secret never lives in the vault directory as plaintext:
  - Android: AES-GCM wrap in Android Keystore; ciphertext in app-private storage.
  - Windows: a 0600 bind file under the user data directory (outside the vault), plus Credential Manager when available.
  - Linux: the same 0600 bind file; libsecret / Secret Service when present.
  - Tests and headless CI: `NEMO_BIND_DIR` holds per-salt files named `device-<salt hex>`.
  - The Compose shell on Android passes the Keystore-unwrapped secret across UniFFI. Desktop passes an empty buffer and `nemo-core` loads the OS secret.
- Version-1 vaults (passphrase-only) still open. New vaults are version 2. Biometric gating of the Keystore key is optional UX on the same wrap; it is not required to unlock.
- **Minimum length:** 8 Unicode scalar values. Empty and shorter passphrases are refused.
- **KDF:** Argon2id, RFC 9106. Parameters are stored **unencrypted** next to the ciphertext so they can be raised later:

  | Parameter | v1 value |
  | --- | --- |
  | Memory | 65536 KiB (64 MiB) |
  | Iterations | 3 |
  | Parallelism | 1 |
  | Salt | 16 random bytes |
  | Output | 32 bytes |

- **Cipher:** SQLCipher (SQLCipher 4 defaults) via `rusqlite`. The mixed 256-bit key is supplied as a **raw key** (`PRAGMA key = "x'<64 hex>'"`). SQLCipher's own PBKDF2 is not used. Version 1 used Argon2id output alone; version 2 XORs the device secret.
- **Layout:** a vault is a directory:

  ```text
  <dir>/kdf.cbor     unencrypted {version, salt, m_cost, t_cost, p_cost}
  <dir>/store.db     SQLCipher database
  ```

  Copying those two files to another device does not unlock a version-2 vault, even with the passphrase. The device secret stays in the OS store.

  `kdf.cbor` is canonical CBOR. `store.db` holds an opaque installation snapshot (identity secret, revocation **public** key, libsignal store including sessions and PQXDH one-time / used Kyber markers, prekey counters, home-binding `seq`) plus MLS group sidecars, pending joins, and minted bound invites.
- **Never stored:** the revocation mnemonic or revocation private key (ADR-0004). Lost passphrase loses the installation; that is the same cost as losing the device (ADR-0003, ADR-0023). The vault is not a backup and is not copied to the home server.
- **Wrong passphrase:** open fails closed. The client MUST NOT distinguish "wrong passphrase" from "corrupt file" in UX copy beyond a single unlock-failed state.
- **Compose / UniFFI:** the shell passes the directory path, passphrase, and (on Android) the Keystore-unwrapped device secret. It MUST NOT write identity, ratchet, or MLS keys of its own.

## Pros

- One unlock model on Android, Linux, and Windows.
- Argon2id is the current password-hashing baseline; SQLCipher is the library ADR-0028 already named.
- Raw-key SQLCipher keeps KDF policy in this project, not in SQLCipher's default iter count.
- Matches "device is the only place plaintext exists" (ADR-0023): ciphertext at rest is still on that device.
- Version-2 bind stops a copied vault directory from opening on a second machine.

## Cons

- A forgotten passphrase is a dead identity. Users who wanted "PIN plus phone backup" will not get it.
- 64 MiB Argon2id is noticeable on low-end Android and at every unlock.
- Two files; copying `store.db` without `kdf.cbor` makes the vault unopenable.
- Lost OS-store entry (factory reset, new user profile, wiped credential store) is a dead vault even with the passphrase. That matches "this device is this identity" (ADR-0003).
- File fallback on Linux without Secret Service is weaker than libsecret: a copy of the user data directory plus the vault can still open. The vault directory alone cannot.

## Alternatives considered

### SQLCipher passphrase mode (built-in PBKDF2)

Rejected: weaker than Argon2id at comparable UX cost; iter count would be a silent SQLCipher default.

### Android Keystore / DPAPI / libsecret wrapping the SQLCipher key

Adopted as version 2: mix, do not replace, the passphrase-derived key. Three platform paths, plus a file fallback when Secret Service is missing. A lost device secret is a lost vault; that is accepted (ADR-0003). Biometric-only unlock is not required.

### Single encrypted blob (ChaCha20-Poly1305) instead of SQLCipher

Rejected: ADR-0028 already bound SQLCipher. A second AEAD would be extra cryptography for the same job.

### Empty or machine-generated passphrase

Rejected: an unlocked file on disk is not a vault. A generated secret still needs somewhere to live (the device bind above), and would skip the user passphrase.

### Store the revocation mnemonic in the vault

Rejected by ADR-0004: the phrase exists to kill a stolen device from outside it.

## Consequences

- `nemo-core` grows a vault module. `nemo-server` stays unaware of it.
- Identity creation UX: show the revocation phrase **and** choose a vault passphrase; state that both are unrecoverable.
- MLS groups, contact nicknames, and message display rows share this SQLCipher file (`groups`, `display`, `inbox` keys). The identity snapshot still excludes the revocation mnemonic.
- Raising Argon2 parameters is a `kdf.cbor` version bump plus a re-encrypt; old files keep their stored parameters.

## History

- 2026-09-20 — Accepted. Application passphrase, Argon2id, SQLCipher raw key, mnemonic never stored.
- 2026-09-20 — Amendment 1: MLS group sidecar is persisted in the same SQLCipher file (`groups` key). Pending joins and minted bound invites are also vault-backed. The identity snapshot still excludes the revocation mnemonic.
- 2026-09-21 — Amendment 2: new vaults mix a device-held secret into the SQLCipher key (`kdf.cbor` version 2). Version 1 files still open.
- 2026-09-25 — Amendment 3: decrypted message display rows and per-conversation `next_seq` live under vault key `inbox` (CBOR); reopen loads them with the vault.
