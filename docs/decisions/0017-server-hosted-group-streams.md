# ADR-0017: Server-hosted MLS group streams with server-side fan-out

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 7, 17, 19, 38, 43, 53.10, 60

## Context

MLS (RFC 9420, RFC 9750) requires that all members of a group agree on exactly one Commit per epoch. Proposals must arrive before the Commit that references them. Some component must therefore impose a linear order on handshake messages and break ties when two members commit concurrently.

The original draft described the delivery service as fully blind. A sequencer for MLS conflicts with that to some degree: either the server holds a per-group stream and delivers it to a known set of member mailboxes, or clients fan out themselves to per-member capabilities and resolve forks client-side.

## Decision

Each MLS group is hosted on **one server** (the creator's home server at creation time) as an **ordered opaque stream**.

- The hosting server assigns a monotonically increasing sequence number to every object appended to the group stream (Proposals, Commits, Welcome, application messages, and the host-framed objects below). Group file bytes are one host-side object reserved by **`AttachmentReserve`**, not a fanned-out envelope (ADR-0019). The host does not parse or validate MLS content beyond framing (RFC 9750 section 5.3).
- An MLS Remove is appended as one host-framed **`RemoveBundle`**: `{opaque_mls_commit, sidecar}` under a single member credential (ADR-0018). The host admits both or neither. It **MUST NOT** parse `opaque_mls_commit` (RFC 9750 §5.3). On admit it **fans the object out to the pre-revoke set (including the named credential)**, then revokes that live `credential_id` and **removes that member's fan-out entry**. Re-add is a new admit only. Other MLS objects (Proposal, Update, Add, application message) stay unframed and do not revoke credentials. Clients reject an unframed Remove and reject a bundle that is not exactly one matching Remove.
- Signing-key rotation uses a host-framed **`SigningKeyReplace`** (ADR-0018). The host **MUST NOT** learn a new filter key by parsing an MLS Update.
- Clients apply the first valid Commit for an epoch in stream order and ignore later ones. A committer waits until its Commit is echoed back in the stream before applying it.
- The hosting server holds the set of **member delivery capabilities** for the group and fans out every appended object to each member's home mailbox using the server-to-server primitive (ADR-0016). Members on other servers do not connect to the hosting server.
- A member **MUST** refresh their fan-out entry when their home server or delivery capability changes: `{delivery_capability, home_server}` is replaced under that member's **member credential**. This is not group migration. The host does not learn the identity.
- Stream append is authorised by a **member credential** issued on admit, not by a mailbox delivery capability (ADR-0018). The one exception is a **host-only** append of a verified `RevocationStatement` (ADR-0004). That object is not an MLS message. The host **MUST NOT** append MLS objects without a member credential. Invites and admits are client-signed; the host cannot mint them. A stolen inbox token cannot change membership or append to the stream.
- Join is invite (signed) → accept (pending join, no fan-out) → admit (signed). The mailbox is added to fan-out only after admit. MLS Add / Welcome travels with admit. Nobody can Add another identity's leaf without that identity's client.
- The hosting server therefore knows, for each group it hosts: the group's opaque id, the set of member mailboxes (capabilities plus home servers), the timing and padded size of every object, and which member credential appended it (the append is authenticated as a member credential, not as an identity; see ADR-0009 sealed sender). It does not know identities behind capabilities beyond what the capability reveals, and it cannot read content.

## Pros

- Correct MLS semantics with minimal client complexity: no client-side fork resolution.
- Bandwidth-efficient: a sender uploads once; the server fans out.
- Sequence numbers give clients gap detection and idempotent acknowledgement per group.
- Offline members receive the full ordered backlog from their own mailbox.
- Revocation statements (ADR-0004) can be pushed into the stream so all members learn of them.

## Cons

- The hosting server learns the group's member mailbox set. Group membership is **not** hidden from infrastructure in v1; README section 19 must say so plainly.
- The hosting server is a single point of failure for the group. Moving a group to another server requires an explicit migration protocol (deferred).
- A malicious hosting server can drop or delay Commits and thereby stall the group (denial of service, not confidentiality loss).
- Filtering "redundant" Commits server-side is intentionally not done; all Commits are delivered and clients decide, which costs some bandwidth but avoids the server desynchronising from the group's real epoch.

## Alternatives considered

### Client-side fan-out to per-member capabilities

Sender uploads N copies. Server never holds a membership set. Rejected: N× upload cost, requires a client-side total-order rule that is fragile under concurrency, and still leaks membership through timing correlation.

### Server-side Commit filtering by epoch

Server rejects Commits for stale epochs. Rejected per RFC 9750 section 5.3 risks: an invalid Commit accepted by the server desynchronises server and clients.

### Host parses MLS Remove to drop the member credential

Rejected: RFC 9750 section 5.3. The host stays a dumb stream. Credential revoke is a field on `RemoveBundle`, not an MLS parse.

## Consequences

- Server data model gains `Group` (opaque group id, hosting server, member capability set, member credentials, stream cursor, retention).
- README section 19 is rewritten to state what the hosting server can observe.
- Acknowledgement semantics (README section 38) distinguish transport ack, protocol ack and user receipt.
- Anonymous group membership remains a deferred feature (README section 52).
- Hybrid join and the delivery / member-credential split are specified in ADR-0018.

## History

- 2026-09-19 — Accepted. Server-hosted MLS group streams with server-side fan-out.
