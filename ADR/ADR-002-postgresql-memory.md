# ADR-002: PostgreSQL Memory

Status: Accepted and implemented as Phase 4 baseline

Use PostgreSQL as the durable Memory engine, with optional pgvector-compatible repositories. Provider memory is not authoritative, and application operational data remains logically and operationally separate.
