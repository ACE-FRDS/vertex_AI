# Vertex AI

Vertex AI is a headless, local-first intelligence engine for Vertex applications. It owns durable context and orchestration while treating every LLM as a replaceable reasoning provider.

This repository deliberately separates project material from implementation:

- Project documentation and decisions live at the repository root.
- Rust implementation and CI live under `ProgramSource/`.

## Current delivery

Architecture Redesign Specification v0.2 has been translated into an executable Phase 0–5 foundation:

- Rust workspace, common configuration, error model, structured logging, tests, and CI.
- Transport-neutral command dispatch with a deterministic mock provider.
- Provider abstraction and provider/model registries.
- Secret-store abstraction and a non-persistent test implementation.
- Windows Credential Manager production adapter.
- OpenAI Responses API adapter with model discovery, generation, privacy enforcement, and mock-HTTP contract tests.
- PostgreSQL Memory schema, migrations, scoped CRUD/full-text search, optimistic concurrency, mutation audit, and a pgvector-compatible embedding boundary.
- Memory Proposal/Permission validation before repository writes.
- Exact-scope/category Memory deduplication and optimistic update conflict detection.
- Context Builder with privacy filtering, strict context budgets, and non-forgeable prepared-context boundaries.
- Provider-neutral Vertex Context Protocol v0.1 domain types.

Advanced retrieval ranking/vector search, automatic routing, additional providers, Edge Core, Management UI, and VXN execution remain later phases and are not represented as complete.

## Build

From `ProgramSource/`:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

See `ARCHITECTURE.md`, `CONTEXT_PROTOCOL.md`, `POSTGRESQL_MEMORY_DESIGN.md`, `SECURITY_MODEL.md`, and `DEVELOPMENT_PLAN.md` for the design baseline.

PostgreSQL integration tests run when `VERTEX_AI_TEST_DATABASE_URL` is set. Unit and boundary tests do not require a database.

The Windows credential adapter has an opt-in live test. This host currently rejects Credential Manager writes with platform error 8; Vertex AI reports the store unavailable and does not fall back to plaintext or mock storage.
