# ADR-004: Secret Storage

Status: Accepted; Windows adapter implemented

Persist production secrets only in the operating-system secret store through `SecretStore`. Plaintext database/config persistence and silent fallback are prohibited. The in-memory implementation is limited to tests and development. Windows uses Credential Manager; macOS and Linux adapters remain pending.
