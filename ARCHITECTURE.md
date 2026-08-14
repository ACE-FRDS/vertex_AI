# Vertex AI Architecture Baseline

Status: Phase 0–5 implementation baseline  
Source: Architecture Redesign Specification v0.2

## Product boundary

Vertex AI is a persistent intelligence engine, not a chat product. It supplies model selection, provider connectivity, context, memory, and future execution support to Vertex applications. Its Core must continue running without the Management UI, and application presentation remains owned by each application.

```text
Application / Management UI / Future SDK
                 |
         Transport Adapter
                 |
    Transport-neutral Command Layer
                 |
           Vertex AI Core
      /          |          \
 Provider    Context       Memory
 Registry    Boundary      Boundary
      |                     |
 Adapters              PostgreSQL
```

## Existing architecture and reusable components

The initial repository contained no implementation. There were therefore no legacy modules to migrate or discard. The Phase 0–2 workspace is the first reusable foundation.

## Rust workspace design

Dependencies point inward and do not form cycles:

```text
vertex-ai-types
    ^        ^
    |        |
provider   secrets
    ^        ^        memory
     \       \        /
           vertex-ai-core
```

| Crate | Responsibility |
|---|---|
| `vertex-ai-types` | Provider-neutral IDs, model metadata, generation DTOs, Vertex Context v0.1 |
| `vertex-ai-provider` | `ModelProvider`, provider/model registries, mock adapter |
| `vertex-ai-provider-openai` | OpenAI model discovery and Responses API wire adapter |
| `vertex-ai-secrets` | Redacted secret value and secret-store interface |
| `vertex-ai-memory` | Scoped memory domain, policy service, PostgreSQL repository and migrations |
| `vertex-ai-context` | Privacy-filtered retrieval, token budgeting, prepared-context capability |
| `vertex-ai-core` | Configuration, error translation, logging, command dispatch, lifecycle |

Transport adapters, PostgreSQL repositories, real providers, Edge Core, VXN, and Management UI will be separate crates or packages when their phases begin.

## Core command architecture

`VertexAiCore::execute(Command)` is the application boundary. Commands contain domain values rather than HTTP, JSON-RPC, IPC, or UI types. Current commands discover/list models, select a model, manage provider-secret references, propose/recall Memory, build Context, and reason. `Reason` performs Context construction and generation inside one Core operation so stateless transports cannot bypass the privacy gate. Future command families will add broader task execution and VXN generation without changing transport ownership.

Provider registration is a trusted lifecycle operation rather than a remotely serializable command. Secret-bearing commands are intentionally not serializable, and logging records only a static command name.

## Provider adapter design

`ModelProvider` exposes provider identity, capabilities, model discovery, generation, streaming, health, and cost estimation. Provider-specific request types and authentication must remain inside adapter crates. A new adapter registers through `ProviderRegistry`; Core behavior does not branch on provider brands.

The Phase 2 mock is deterministic and supports integration tests without cloud access. Phase 3 adds an OpenAI adapter using `GET /v1/models` and `POST /v1/responses`; requests disable provider-side storage and reject context that is not cloud-allowed.

## PostgreSQL memory boundary

Core calls a provider-neutral memory service/repository interface. Phase 4 implements PostgreSQL migrations, exact-scope CRUD, full-text search, optimistic concurrency, content-free audit events, and an embedding boundary that does not require pgvector. No LLM or provider adapter receives database credentials or repository access. See `POSTGRESQL_MEMORY_DESIGN.md`.

## Vertex Context Protocol

`VertexContext` v0.1 is defined in `vertex-ai-types`. Phase 5 adds a Context Builder that retrieves only exact-scope candidates, applies target-specific privacy rules, and fits selected records into a hard token budget. Core generation accepts only a non-deserializable `PreparedContext` produced by that builder, preventing transports from bypassing the privacy gate with raw context. Provider adapters translate the final envelope only at the provider boundary. See `CONTEXT_PROTOCOL.md`.

## Secret storage

`SecretStore` stores and retrieves opaque values using stable secret identifiers. `SecretValue` redacts `Debug` output and zeroes its owned string on drop. The in-memory adapter is explicitly test/development-only. Phase 3 implements Windows Credential Manager without plaintext fallback; macOS Keychain and Linux Secret Service adapters remain pending.

## Model router

Phase 2 implements manual selection only. Model Registry is independent of selection state, so adding rule-based Auto and evidence-based Council modes does not require changing provider adapters. Auto routing will score capability, privacy, locality, availability, latency, cost, and historical performance. Council synthesis must preserve disagreement and evidence rather than perform a simple vote.

## Transport boundary

Transport adapters map inbound authentication and DTOs to `Command`, then map `CommandResponse` and `CoreError` outward. The Core does not import an HTTP, gRPC, IPC, or UI framework. Authorization is enforced before command execution and again by sensitive domain services where appropriate.

## Edge Core boundary

Edge Core will depend on shared domain/command contracts, not on the central service process. Its local queue records idempotency key, application scope, operation, created time, retry state, and policy. Replay occurs only after health recovery and validation. An unavailable central Core must degrade AI capability, never stop the host application's business operations.

## Management UI boundary

The Vue 3 + TypeScript Management UI is a separate client for configuration, diagnostics, monitoring, and the AI Test Console. It has no privileged database or provider access. It must not become a general chat product, and stopping it must not affect Core execution.

## VXN integration boundary

Future VXN integration is a pipeline of generator, validator, permission evaluation, and isolated runtime. Generated VXN is data until validation succeeds. Core commands will depend on a versioned VXN boundary interface, never directly on runtime internals.

## Security model

LLM, memory, secrets, application data, files, network, OS, and VXN execution are separate trust zones. Capabilities are explicit, scoped, and deny-by-default. See `SECURITY_MODEL.md`.

## Test strategy

- Unit tests: identifiers, protocol serialization, redaction, registries, adapter behavior.
- Integration tests: command → registry → provider and secret command → store.
- Later contract tests: every real provider and OS secret adapter.
- Later database tests: migrations, scope isolation, conflict/deduplication, retrieval.
- Later failure tests: central Core outage, Edge queue/replay, idempotency.
- Quality gates: format, Clippy with warnings denied, all workspace tests.
