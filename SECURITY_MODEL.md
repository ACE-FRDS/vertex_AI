# Vertex AI Security Model

## Trust zones

Caller, Management UI, transport, Core, provider adapters, external LLMs, Memory Core, secret store, application data, filesystem/network tools, Edge Core, and VXN runtime are distinct zones.

## Rules

- Deny by default; grant named capabilities for a scoped principal and operation.
- Authenticate at transport ingress and authorize against application/project/user scope.
- Recheck scope in memory, secret, tool, and VXN services.
- Never expose a `SecretValue` through a response, serialization, debug output, metric, backup, or error report.
- Never send `local_only`, sensitive-without-policy, or non-cloud-allowed context to a cloud provider.
- Never allow provider adapters or LLM output direct database/application-data access.
- Treat model output, generated tool calls, and VXN as untrusted input requiring validation.
- Log identifiers, timing, counts, and decisions; do not log prompt, memory body, secret, or private response by default.

## Implemented controls

Secret-bearing commands are internal non-serializable values, command spans log only static operation names, and secrets are zeroized on drop. The included in-memory store is not a production fallback. Windows production storage uses Credential Manager and explicitly refuses unsupported platforms. Cloud providers reject non-cloud-allowed or local-only context before HTTP transmission. Memory proposals require an exact-scope write permit before repository access, and SQL always repeats exact scope predicates. Context construction rejects privacy laundering, filters each Memory before budgeting, and returns a non-deserializable prepared-context capability; Core rejects using it with a model in a different local/cloud location.

## Threat-driven tests

Tests must cover secret redaction, cross-scope memory denial, cloud privacy rejection, malicious provider error content, context-budget overflow, replay duplication, VXN capability escalation, and logs/backups free of protected content.
