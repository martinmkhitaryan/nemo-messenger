# ADR-0018: Hybrid group join and split member credentials

- **Status:** Accepted
- **Date:** 2026-09-19
- **Affects:** README.md sections 17.1, 19, 20, 22, 43, 53.10

## Context

Security review found two holes: (G1) if a group delivery capability also authorises fan-out changes, a stolen inbox token can alter membership; (G2) any member holding a contact card could MLS-Add someone without their consent.

## Decision

### Token split

- A **delivery capability** (ADR-0008) authorises append to a mailbox only. It never authorises group-stream append or fan-out changes.
- A **member credential** is **reserved** on accept and **activated** when the pending join is **admitted**. Only the live credential authorises append to *that* group's stream, group-file fetch, and fan-out refresh. It is not shared with other members, is not a delivery capability, and cannot mint invites.
- MLS Remove is one stream object, **`RemoveBundle`**: `{opaque_mls_commit, sidecar}` appended under **one** member credential. The sidecar is `{credential_id, "revoke"}` signed with the committer's **group member signing key**. It is not MLS. The host **MUST NOT** parse `opaque_mls_commit` (RFC 9420 / RFC 9750 §5.3).
- That Commit **MUST** process **exactly one** Remove, of the leaf bound to `sidecar.credential_id`. RFC 9420 allows multi-Remove Commits; this application does not. To remove N members, append N `RemoveBundle`s. Clients **MUST** reject a bundle whose Commit removes zero, two or more, or a different leaf (RFC 9750 §5.3: clients reject Commits that violate group policy).
- Clients **MUST** reject an **unframed** Commit that processes a Remove. A Remove that is not a `RemoveBundle` **MUST NOT** be applied. Otherwise a member can be ejected in MLS and keep a live credential.
- On admitting a `RemoveBundle` the host **MUST**, in this order: verify the sidecar against the **current** registered signing key of the **appending** credential; **fan-out the object to the pre-revoke member set, including `sidecar.credential_id`** (the removed member must receive the Commit that removes them); then, with no grace, revoke that **live** `credential_id` and **remove that fan-out entry**. A pending reserved id is not live and is not revoked this way. A loose sidecar, a sidecar on an Update/Add, or a Commit without a sidecar **MUST NOT** revoke. Orphans do not exist: it is one append.
- Clients **MUST** apply the MLS Remove if and only if `sidecar.credential_id` is the removed leaf's host credential (the MLS-published id, ADR-0018 leaf extension). On mismatch they **MUST** reject the bundle (do not apply the Commit) and alert. Recovery is a new admit. A live member who can append a `RemoveBundle` can infra-eject a named credential; that is the same power class as MLS Remove. Members still apply RFC 9420 Remove for cryptographic membership.
- Stealing an inbox delivery capability cannot Add, Remove, append to a group stream, or mint invites.

### Member signing keys

- Each member holds a **per-group signing key** created by their client at admit. The public half **MUST** be published **inside MLS** before that member invites or admits. A copy is registered with the host for filtering only. The host copy is a hint. The identity key is never sent to the host.
- Clients **MUST** reject invite and admit signatures that do not verify against the MLS-published key. If the host's copy and the MLS copy disagree, the MLS copy wins and clients alert (review H3, H4).
- Rotation is an MLS Update **and** a host-visible **`SigningKeyReplace {new_public_key}`** signed by the **currently registered** key, appended under **that member's** credential. The host replaces **only** that credential's filter key. The old host key **MUST NOT** verify afterwards. There is no host API that parses an MLS Update (RFC 9750 §5.3). Members **MUST** publish the same new key in MLS. Clients still prefer the MLS copy.

### Hybrid join

- A current member **invites** by creating a one-time, short-TTL **group-invite** (same consume/TTL rules as ADR-0007) signed with their **group member signing key**: `{group_id, nonce, ttl, invitee_binding?}`. The host only stores and consumes that object. It cannot mint a valid invite.
- If the inviter already has the invitee's contact card, the invite is **bound**. `invitee_binding` is a **commitment only**: `H(identity_public_key || salt)`. The salt may sit on the host. The host-visible invite **MUST NOT** contain `identity_public_key` or `identity_id`. The invitee proves they match that binding **to the admitting member inside the encrypted session**, not to the host: an identity-key signature over `{group_id, pending_id}`. The admitter checks the proof under the pinned key and that `H(that key || salt)` equals `invitee_binding`. The admitter **MUST NOT** admit if the proof is missing or fails. The host does not verify the proof (review B1, H3b). Stranger invites are unbound and treated like a password.
- The invitee **accepts**. The host records a **pending join**: reserved delivery capability, a **reserved `credential_id`**, no fan-out, no stream append, no fetch, no invite minting. The reserved id is inert. The invite is consumed. The accept response and the host-visible pending object include that reserved `credential_id` (not an identity).
- The invitee's KeyPackage **MUST** carry a leaf extension with that reserved `credential_id` (RFC 9420 leaf extensions; the host **MUST NOT** parse it). The admitter **MUST NOT** admit if the extension is missing or does not equal the pending reserved id.
- A current member **admits** with a signature of `{group_id, pending_id, "admit"}` under their group member signing key (typically with the MLS Add / Welcome). For a **bound** invite the inviter **MUST** be the admitter, or the inviter **MUST** relay the identity-key proof and KeyPackage to the admitter over MLS. The bound proof **MUST NOT** appear in any host-visible accept field (review PJ1). Immediately before admit, the admitter **MUST** fetch discovery for the invitee's identity and **MUST NOT** admit if a valid `RevocationStatement` is present (review RA1). This is application policy; MLS still allows Add. Members verify the admit signature against the MLS-published key. Only then does the host **activate the same reserved `credential_id`** (it **MUST NOT** mint a different id), register the joiner's new group signing public key, and **add the mailbox to fan-out before** any admit-associated stream object is fanned. If Welcome is sent in-band to the joiner instead, that is sufficient (RFC 9750 §5.2.3).
- Clients **MUST** treat the MLS-published `credential_id` as that leaf's host credential. The host-visible pending id is a hint; MLS wins on disagreement. That extension is **immutable**: clients **MUST** reject an Update or other Commit that removes or changes it.
- Clients **MUST** reject External Commits, external joins, and MLS ReInit. Join is the hybrid path only. Group re-form after a dead host is deferred (section 52). RFC 9420 defines those messages; this application does not use them (RFC 9750 §5.3).
- Pending joins expire with the invite TTL if no admit arrives.
- Creating a group is a self-admit: the host activates the creator's `credential_id` and registers the first group signing key. The create response includes that id. The creator **MUST** publish it in the founding MLS leaf extension. The host already knows that mailbox as the creator.
- A connect token (ADR-0007) is not a group invite. Adding an existing contact still requires they accept a (bound) group invite.

### KeyPackages

- MLS KeyPackages used for join are produced at accept and consumed on admit, not left infinitely fetchable by anyone who once had a card.
- The KeyPackage travels **in-band to the admitting member**, or is sealed to members. The host **MUST NOT** store it in the clear. It may keep an opaque blob only. `identity_id` on a KeyPackage is client or home-server bookkeeping, not host state (review KP1).

## Pros

- Consent: you cannot be dropped into a group you never saw.
- Stolen mailbox tokens do not take over group membership.
- The joiner registers themselves. Invite and admit signatures use group member keys, so the host never sees the identity key (ADR-0009).
- A malicious host cannot mint invites that **members** accept: they verify against the MLS-published key.
- Known-contact invites cannot be redeemed by a different identity; the host does not learn that identity.

## Cons

- The invitee must open the invite, then a member must be online to admit. Offline invitees or inviters wait; the pending join may expire.
- Unbound (stranger) invites remain a race: whoever redeems first joins as themselves.
- The host still sees the eventual member-capability set. It can still drop or delay objects (DoS, not membership forgery).
- More credentials, pending-join state, and one signing key per group. The host still sees that signing public key as a stable per-member handle.
- Clients that ignore the MLS-published key and trust the host filter reopen H3. Conformant clients MUST NOT do that.

## Alternatives considered

### Unilateral Add by any member who has a contact card

Rejected: no consent; stolen or old cards become a group-Add oracle.

### Admin-only invites

Compatible later; not required for v1. Hybrid already requires the joiner's accept.

### Same token for mailbox and group host

Rejected: G1.

### Host-issued invite tokens (authorised only by a member credential)

Rejected: a malicious host already holds every credential it issued, so it can mint invites and expand fan-out (review H1).

### Fan-out immediately on accept

Rejected: joiner would be on the member set before MLS Add, and could append or mint if credentials were live (review J1).

### Always-unbound group invites

Rejected for known contacts. Binding when the inviter has a card closes the interceptor-joins-as-themselves case (review I2). Stranger invites stay unbound.

### Sign invite and admit with the identity key

Rejected: the host would learn who invited and who was named (review H3).

### Host verifies the bound-invite proof

Rejected: the host would learn the invitee's identity (review H3).

### Store the KeyPackage in the clear on the group host

Rejected: the host would learn `identity_id` at accept (review KP1).

### Put `identity_public_key` or `identity_id` in `invitee_binding`

Rejected: the host would learn who was named (review H3b).

### Put the bound-invite proof in a host-visible accept object

Rejected: the host would learn the invitee's identity (review PJ1). The inviter admits, or relays proof and KeyPackage over MLS.

### Host parses MLS Remove to drop the member credential

Rejected: RFC 9750 section 5.3. Use `RemoveBundle`; the host never reads the Commit.

### Loose sidecar on any Commit

Rejected: the host cannot tell Remove from Update (review SC2). Revoke only on `RemoveBundle`.

### Host parses MLS Update to refresh the filter signing key

Rejected: RFC 9750 section 5.3. Use `SigningKeyReplace`.

### Multi-Remove in one Commit

Rejected for this application: one sidecar names one `credential_id`. N removals are N `RemoveBundle`s. Clients reject a multi-Remove bundle (RFC 9750 §5.3).

### Fan-out prune before delivering the RemoveBundle

Rejected: the removed member would not receive the Commit that removes them (RFC 9420). Fan-out first, then revoke and prune.

### Issue a new credential_id on admit

Rejected: the KeyPackage already bound the reserved id into the leaf. Admit activates that id.

### Allow the leaf `credential_id` extension to change

Rejected: RemoveBundle matching would become unimplementable. The id is immutable for the life of the leaf.

### Add the joiner to fan-out after the Welcome is appended

Rejected: the joiner would miss Welcome (RFC 9750 §5.2.3). Fan-out first, or send Welcome in-band.

## Consequences

- README 17.1 is invite (signed) → accept (pending) → admit (fan-out + credential). Bound `invitee_binding` is a hash commitment. Admit requires the in-band proof and a fresh discovery check. Host drops the member credential on admitted Remove.
- README 19: the host sees member credentials and delivery capabilities, still not identities. KeyPackages are not host-visible plaintext.
- Anonymous membership remains deferred.

## History

- 2026-09-19 — Accepted. Hybrid group join and split member credentials.
