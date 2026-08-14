# Vertex Context Protocol v0.1 Draft

## Purpose

Vertex Context Protocol is the provider-neutral envelope used to supply relevant understanding to interchangeable models. It is not a conversation transcript and is not a database serialization format.

## Envelope

```json
{
  "vertex_context": "0.1",
  "task": {},
  "application": {},
  "project": {},
  "user_context": {},
  "memories": [],
  "decisions": [],
  "constraints": [],
  "schema": {},
  "tools": [],
  "permissions": {},
  "runtime": {},
  "privacy_policy": {
    "local_only": false,
    "cloud_allowed": false,
    "sensitive": false,
    "share_scope": null
  }
}
```

Unknown data is carried as structured JSON during v0.1 while stable sub-schemas are learned. The envelope itself is typed and versioned in Rust. A breaking semantic change requires a new protocol version.

## Invariants

1. Context is built for one task and one candidate model.
2. Scope and permission filters run before relevance ranking.
3. `local_only` or non-cloud-allowed content never reaches a cloud adapter.
4. Memories are selected; the full memory store is never inserted.
5. Provider formatting happens only inside an adapter.
6. Secrets, database credentials, and hidden authorization state never enter the envelope.
7. Token budget is a hard output of context construction, not an advisory label.
8. Stateless clients use the Core `Reason` operation; raw or round-tripped JSON cannot be promoted into a prepared generation context.

## Implemented context builder baseline

Validate scope and budget → reject protected base-context laundering → retrieve exact-scope candidates → enforce local/cloud and sensitive-memory privacy → reserve response tokens → fit records by retrieval order without exceeding the budget → issue a non-forgeable prepared-context value. Advanced semantic, dependency, and model-tokenizer ranking remains later work.
