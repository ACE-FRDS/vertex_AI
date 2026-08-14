# PostgreSQL Memory Core Design

Status: Phase 4 baseline implemented.

## Boundary

Memory is owned by Vertex AI and is independent from provider memory and application operational databases. PostgreSQL is accessed only through `MemoryRepository`. LLM-derived values enter through `MemoryProposal` and require a trusted `MemoryWritePermit`; they never receive SQL or direct repository capability.

The initial migration is located at `ProgramSource/crates/vertex-ai-memory/migrations/0001_memory_core.sql`.

## Initial logical records

The first migration models:

- stable memory ID and category;
- system/organization/user/application/project/session scope columns;
- content plus structured JSON content;
- priority and confidence with database constraints;
- source and provenance;
- created, updated, and optional expiry timestamps;
- privacy policy and extensible metadata;
- version for optimistic concurrency;
- optional embedding reference through a pgvector-compatible abstraction.

Relations, embedding storage, and content-free mutation audit are separate tables so basic records do not require vector support.

## Write pipeline

The implemented baseline is proposal → schema validation → exact-scope permission → sensitive-memory permission → approved write → content-free audit event. Conflict detection and deduplication policy remain next-stage work. Database mutation and audit insertion share one transaction.

## Retrieval pipeline

Mandatory exact-scope SQL predicates run before PostgreSQL full-text filtering and priority/confidence/recency ordering. Semantic/vector and relation ranking will be added by the Context phase. Raw private content is excluded from logs.

## Isolation and operations

- Dedicated database/schema and credentials separate Vertex Memory from application data.
- Row-level security may reinforce, but never replace, application authorization.
- Migrations are forward-versioned and tested on clean and upgrade databases.
- Backup/export is versioned and excludes secrets.
- pgvector use is optional behind an embedding repository so local-first operation remains possible without it.
