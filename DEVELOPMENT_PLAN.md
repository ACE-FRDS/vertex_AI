# Development Order and Acceptance Gates

## Completed scope

1. Phase 0: Rust workspace, configuration, error/logging baseline, unit/integration tests, CI, architecture documents, ADRs.
2. Phase 1: transport-neutral commands and deterministic Mock Provider command flow.
3. Phase 2: provider interface, provider/model registries, secret abstraction, development secret adapter.
4. Phase 3: OpenAI model discovery/generation adapter and Windows Credential Manager adapter. The adapter compiles and fails closed; this development host returned `PlatformFailure(Error 8)` during the opt-in live credential test, so host-level validation remains open.
5. Phase 4: PostgreSQL migration, scoped CRUD/full-text search, Memory Proposal permission boundary, and cross-model memory regression test.
6. Phase 5 baseline: privacy-filtered Context Builder, context budget, protected-base rejection, prepared-context generation boundary, and atomic Build+Generate `Reason` command.

## Next order

1. Complete Phase 4 hardening: run migration/CRUD tests against PostgreSQL in CI; add deduplication and conflict policy.
2. Complete Phase 5: semantic/dependency ranking, provider tokenizer adapters, protocol fixtures, and context diagnostics.
3. Phase 6–7: multi-provider switching and Ollama/local provider; prove the cross-provider memory test with two real adapters.
4. Phase 8–9: thin Vue Management UI and rule-based Auto Router.
5. Phase 10–12: Edge queue/recovery, Council prototype, VXN boundary.

## Per-phase gate

Design and threat review → implementation → format/build → unit tests → integration tests → failure tests → measurement → analysis → regression test. A phase is not complete when required tests are skipped or when later-phase behavior is only mocked and presented as finished.

## Milestone tests

- Provider setup: secret registration → provider connection → discovery → Model Registry.
- Test Console: client command → selected Model A → response.
- Persistent memory: save Project Alpha fact with Model A → switch to Model B → retrieve PostgreSQL answer from Vertex Memory.
- Edge failure: stop central Core → host continues → task queued → Core returns → validated idempotent replay.
