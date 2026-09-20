# Protocol specifications

Each development phase in [ADR-0027](../decisions/0027-protocol-first-development-order.md) ends with a document here. A later phase does not start until the previous document exists and agrees with the decision records. Prototypes are allowed; a prototype's API or schema is not a specification.

The product README (`../../README.md`) remains the architectural specification. These files are the **phase-complete** protocol documents that an independent implementation or audit can read without reconstructing the argument from the README and the decision log.

| Phase | Document | Status |
| --- | --- | --- |
| 1. Security model | [01-security-model.md](01-security-model.md) | Complete |
| 2. Cryptographic protocol | [02-cryptographic-protocol.md](02-cryptographic-protocol.md) | Complete |
| 3. Envelope protocol | [03-envelope-protocol.md](03-envelope-protocol.md) | Complete |
| 4. Delivery protocol | [04-delivery-protocol.md](04-delivery-protocol.md) | Complete |
| 5. Federation | [05-federation.md](05-federation.md) | Complete |
| 6. Privacy transport | [06-privacy-transport.md](06-privacy-transport.md) | Complete (v1 subset; cover/constant-rate deferred) |
| 7. Application protocol | [07-application-protocol.md](07-application-protocol.md) | Complete (v1 message types) |
| 8. API and persistence | [08-api-and-persistence.md](08-api-and-persistence.md) | Complete (HTTP + DDL + sqlx) |

Languages and libraries for *this* implementation: [ADR-0028](../decisions/0028-implementation-languages-and-libraries.md). Routes and tables: [ADR-0033](../decisions/0033-local-http-api-and-postgres-schema.md). Crates: [`crates/nemo-wire`](../../crates/nemo-wire) (MIT encodings), [`crates/nemo-server`](../../crates/nemo-server) (MIT delivery, local HTTP, S2S mTLS), [`crates/nemo-core`](../../crates/nemo-core) (AGPL identity, PQXDH, MLS, mailbox envelopes), [`crates/nemo-ffi`](../../crates/nemo-ffi) (AGPL UniFFI). Shell: [`apps/compose`](../../apps/compose).
