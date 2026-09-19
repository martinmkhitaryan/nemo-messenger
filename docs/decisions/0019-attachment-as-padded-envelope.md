# ADR-0019: Attachments — 1:1 mailbox envelope; groups keep one encrypted object on the host

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 34, 43, 44.1, 46.4, 53.12

## Context

A public blob store with a fetch URL becomes an archive and a correlation handle. Fan-out of the file into every group mailbox (the first version of this record) avoids a fetch API but copies up to 16 MiB onto every member, which is an availability attack. 1:1 already has a single copy in the recipient mailbox.

## Decision

There is **no public blob CDN, no content-addressed archive, and no file key on any server**.

- Filename, MIME type, thumbnails and file keys live only inside E2EE application plaintext. Nothing about the file is in a URL, header, query string, or outer envelope.
- Ciphertext is padded to a **large size bucket** (ADR-0010). The largest bucket is **16 MiB**. No chunking. No globally meaningful blob id (ADR-0012).
- **1:1:** the padded ciphertext **is** the mailbox envelope, delivered by S2S (ADR-0016), same as text. The recipient fetches it with their mailbox. No second store.
- **Groups:** the host-visible stream object is **`AttachmentReserve`**: `{fetch_token, size_bucket, ttl}` under the uploader's member credential. Caption and file keys stay in the MLS application message. The file ciphertext is **one** object on the **group host**. Members do not receive a mailbox copy unless they fetch. The host **MUST NOT** read `fetch_token` out of MLS plaintext.
- **Group fetch:** the member’s home server requests the object over S2S with `fetch_token` **and** that member’s **live member credential**. Token alone is not enough (removed members and log-leaked tokens cannot pull). The host **MUST** revoke the credential, with no grace, when it admits a `RemoveBundle` for that credential (ADR-0018). Members never connect to the host. The sender’s server does not see the fetch.
- Admitting an `AttachmentReserve` atomically **reserves** `fetch_token → (member_credential, seq)`. Upload **MUST** use that pair. A second reserve of the same token is rejected. `fetch_token` **MUST** be 256-bit CSPRNG. The first successful upload of a reserved token is **immutable**. It sees that some member credential fetched, and when (open-file timing). It must not log the token. Unknown-token responses are constant-time.
- `ttl_bucket` (ADR-0014) applies to the 1:1 envelope and to the group object. Disappearing attachments: short bucket; after display the client deletes the cached file and the message key. The host deletes the object at TTL or when the credential set no longer includes the last eligible fetcher under server policy.
- Large-bucket uploads and fetches are quota’d per capability / member credential and per peer server. A host that swaps the object loses: clients fail closed on AEAD.

## Pros

- No global identifier, no file key on the server, no sender-side download receipt.
- Groups store one copy, not N. 1:1 stays a single mailbox envelope.
- Leavers cannot fetch after their member credential is revoked.
- Sealed sender and S2S unchanged.

## Cons

- The group host sees **who fetched** (by member credential / peer server) and when. One user per server approximates the person.
- Groups have a second object type. 1:1 does not.
- 16 MiB and no chunks: no video-scale files until a later record.
- Coarse large buckets still distinguish small images from max-size files.

## Alternatives considered

### Fan-out the file into every member mailbox

Rejected for groups: N× storage and mailbox eviction (review A2). Kept for 1:1, where N = 1.

### Public or sender-hosted blob URL

Rejected: archive, global handle, and/or sender sees opens.

### Fetch by token alone

Rejected: leaked tokens and removed members could pull ciphertext.

### Content-addressed ciphertext

Rejected: stable handle. Conflicts with ADR-0012.

## Consequences

- README section 34 is normative.
- ADR-0017: group **text** still fans out; group **files** do not.
- Push (ADR-0020) must not carry size or “this is a file.”
- Related: ADR-0015 (1:1 envelopes still share the mailbox bound), ADR-0023 (not a backup).

## History

- 2026-09-19 — Accepted. 1:1 mailbox envelope; groups keep one encrypted object on the host.
