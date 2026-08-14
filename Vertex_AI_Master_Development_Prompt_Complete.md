# Vertex AI — Master Development Prompt
## Complete Rebuild Specification / Codex Implementation Directive

> Status: Master specification
> Purpose: Rebuild the Vertex AI development prompt from the beginning as one coherent source of truth.
> Principle: This document supersedes piecemeal addenda where they conflict. Preserve the architectural intent; implement incrementally and safely.

---

# 1. Mission

Build **Vertex AI** not as another chat application, but as a **local-first AI control plane and intelligent computer-environment manager**.

Vertex AI must sit between the human user, local/cloud AI models, runtimes, applications, developer tools, creator tools, operating-system resources, and future Vertex products.

The product should make complex AI/computer infrastructure understandable and operable without requiring the user to know the difference between a model, runtime, provider, API, PATH entry, service, registry entry, GPU backend, or storage layout.

The core UX promise is:

> **The user describes what they want. Vertex understands what this computer has, what it can do, what is missing, what is broken, and the safest way forward.**

Vertex AI is therefore both:

1. an **AI orchestration/control plane**, and
2. an **intelligent environment layer** that understands the host computer.

It must be designed as infrastructure that other Vertex products can reuse.

---

# 2. Product Philosophy

## 2.1 Human-first abstraction

Never force ordinary users to understand infrastructure merely to use AI.

Internally, Vertex may deal with:

- local LLM runtimes,
- cloud APIs,
- model formats,
- embeddings,
- context windows,
- CUDA,
- GPU drivers,
- environment variables,
- services,
- registry keys,
- ports,
- storage paths,
- dependency versions,
- database servers,
- creator applications,
- developer applications.

Externally, Vertex should translate these into understandable states and actions.

Example:

Instead of:

> ECONNREFUSED 127.0.0.1:11434

prefer:

> Ollama is configured as the current AI provider, but Vertex cannot find a running Ollama service on this computer. OpenAI is available as an alternative.
>
> [Switch provider] [Check Ollama] [Details]

Always preserve the raw technical information under a Details/Developer view.

## 2.2 Facts, interpretation, and action must be separated

Vertex must distinguish:

- **Observed fact** — directly detected state.
- **Inference** — likely explanation derived from evidence.
- **Recommendation** — proposed next action.
- **Mutation** — an actual change to the computer.

Never present AI inference as verified fact.

High-risk mutations must never occur merely because an LLM suggested them.

## 2.3 Reversible by default

Repairs, migrations, cleanup, configuration changes, and model moves should be reversible wherever technically possible.

Use:

- backups,
- transaction-like plans,
- dry-run previews,
- restore points where appropriate,
- rollback metadata,
- audit logs.

## 2.4 Local-first, provider-neutral

Vertex AI must not depend on one model vendor.

Architecture principles:

- Local-first
- LLM-agnostic
- Provider-neutral
- Memory-centric
- Transport-agnostic
- Edge-resilient
- VXN-ready
- Reusable by other Vertex products

Cloud AI is an option, not an architectural dependency.

---

# 3. Product Positioning

Do **not** design Vertex AI as a conventional chat application.

The primary application is a management and intelligence console.

Primary sections should include concepts such as:

- Dashboard
- Models
- Providers
- Routing
- Memory
- Applications
- Environment Explorer
- System Health
- Storage
- Edge Cores
- Security
- System Status
- Logs
- AI Test Console

A conversational assistant may exist as an interaction surface, but it must not define the architecture.

---

# 4. Reference Technology Stack

Prefer the following baseline unless a concrete engineering reason requires change:

### Desktop shell / UI
- Tauri 2
- Vue 3
- TypeScript
- Vite
- Pinia
- Quasar or a similarly maintainable component layer

### Core
- Rust

### Persistent memory / structured state
- PostgreSQL where appropriate

### Security
- OS-native secret storage for API keys and credentials
- Never store secrets as ordinary plaintext configuration

The UI must depend on stable Core APIs/events rather than directly implementing OS-specific discovery logic.

---

# 5. Vertex Edge Core

Create **Vertex Edge Core** as a reusable Rust service layer.

It is responsible for deterministic, high-performance, non-LLM operations.

Responsibilities include:

- filesystem discovery,
- persistent indexing,
- incremental scanning,
- filesystem watchers,
- file hashing/fingerprinting,
- duplicate detection,
- model discovery,
- runtime discovery,
- application discovery,
- dependency discovery,
- storage monitoring,
- process/service health,
- hardware discovery,
- GPU/VRAM/RAM information,
- model registry,
- migration jobs,
- background job queue,
- diagnostics collection,
- safe system inspection,
- structured event emission.

The AI layer must not replace deterministic code where deterministic code can establish the truth.

Rust gathers evidence. AI interprets evidence.

---

# 6. Provider Architecture

Every AI backend must be implemented through a common provider abstraction.

Examples may include:

- OpenAI-compatible APIs
- local runtimes such as Ollama
- LM Studio-compatible endpoints
- other local engines
- future cloud providers
- Vertex-native providers

A provider adapter should expose capabilities rather than forcing every provider into identical behavior.

Capability metadata should cover where applicable:

- chat
- streaming
- tool calling
- structured output
- embeddings
- vision
- audio
- context limits
- model listing
- health/status
- authentication requirements
- local/cloud classification

Adding a provider should not require rewriting Vertex AI.

Prefer:

> adapter/plugin + configuration + capability declaration + registration

over provider-specific branching throughout the codebase.

---

# 7. BYOK and Credentials

Support **Bring Your Own Key (BYOK)**.

The normal flow should be:

1. Select provider.
2. Enter credentials.
3. Store secrets using OS-protected secret storage.
4. Validate connectivity.
5. Discover supported models/capabilities.
6. Make the provider available to routing.

Never expose stored secrets unnecessarily.

Logs must redact credentials.

---

# 8. Model Routing

Support at least three conceptual routing modes:

## Manual
The user explicitly chooses the model/provider.

## Auto
Vertex selects a suitable model according to capabilities, availability, policy, performance, privacy, cost, context size, and task requirements.

## Council
Multiple models may independently reason/evaluate and a coordinating layer combines or judges results where the workflow warrants it.

Routing must be policy-driven rather than hard-coded.

Local and cloud models should be peers under one orchestration layer.

---

# 9. Model Manager

Build a unified **Model Manager**.

Users should be able to:

- discover models,
- register existing models,
- download models through supported runtimes/providers,
- inspect format and metadata,
- select models,
- check compatibility,
- see RAM/VRAM/storage requirements,
- update models where supported,
- remove models safely,
- locate model files,
- identify duplicates,
- move models between storage devices,
- understand which runtimes can use which physical model data.

Do not assume the system drive is the correct storage location.

The user must be able to choose model storage on another drive.

Vertex should remember storage roots and manage them centrally.

---

# 10. Shared Model Library and Duplicate Prevention

A major product goal is to prevent users from unknowingly storing multiple copies of the same large model.

Vertex must detect:

- identical model files,
- equivalent models stored in different runtime layouts,
- duplicate GGUF files,
- runtime-managed blobs/manifests,
- orphaned model data,
- caches,
- stale downloads.

Use hashes/fingerprints plus metadata where appropriate.

Important:

Do **not** falsely promise that Ollama, LM Studio, or every runtime can directly share one physical file.

Instead classify:

- directly shareable,
- importable without re-download,
- convertible,
- runtime-specific copy required,
- unknown/unsupported.

Explain the reason to the user.

---

# 11. Storage & Migration Wizard

Provide a safe **Storage & Migration Wizard**.

Example:

> Your system drive is running low on space. 38 GB of local AI models are stored on C:. D: has sufficient free space.

Possible actions:

- move selected models,
- change default model storage,
- migrate caches,
- preserve runtime compatibility,
- validate copied data,
- update references only after validation,
- roll back on failure.

Never delete the source before destination validation succeeds.

---

# 12. Vertex Environment Explorer

Expand the earlier AI Environment Explorer into a broader **Vertex Environment Explorer**.

Its job is to answer:

> **What is installed on this computer, where is it, what does it do, and what capabilities does it give Vertex?**

Do not limit discovery to AI tooling.

Organize discovered assets into semantic categories such as:

- AI
- Developer
- Creator
- Runtime
- Database
- Server
- System
- Hardware
- Storage

---

# 13. Developer Environment Discovery

Detect and classify developer tooling where possible, for example:

- Python
- Node.js
- Rust toolchains
- Git
- VS Code
- Visual Studio
- compilers
- package managers
- Docker/container tooling
- database clients/servers
- SDKs
- CUDA/toolkits
- AI runtimes

Record useful metadata such as:

- executable path,
- version,
- architecture,
- environment registration,
- active/default version,
- related services,
- related PATH entries.

---

# 14. Creator Environment Discovery

Vertex must also understand creator applications.

Examples of capability classes:

- image editing,
- vector graphics,
- video editing,
- compositing,
- 3D creation,
- audio editing,
- streaming/recording,
- media conversion.

Applications may include software such as Photoshop, Illustrator, Premiere Pro, After Effects, DaVinci Resolve, Blender, Affinity applications, OBS, Audacity, ffmpeg, and future tools.

Do not merely list application names.

Maintain a **capability graph**.

Example:

- Blender -> 3D modeling/rendering
- DaVinci Resolve -> video editing/color/audio
- ffmpeg -> media conversion/transcoding
- VS Code -> source editing/development

This allows Vertex to answer requests based on what the machine can already do.

Example:

> “Make this video smaller.”

Vertex may discover that ffmpeg is installed and propose using it instead of unnecessarily installing another application.

---

# 15. Fast Environment Search

Environment discovery must feel extremely fast.

Take inspiration from the user experience of high-performance file indexing/search tools such as Everything, but do not copy proprietary implementation blindly.

Design for:

- persistent index,
- initial scan,
- incremental updates,
- filesystem watching,
- targeted roots,
- exclusions,
- cancellation,
- low-priority/background scanning,
- fast metadata queries,
- hash jobs separated from lightweight discovery.

Do not repeatedly scan every drive from scratch.

Hash only when required.

---

# 16. AI Asset Search

Provide semantic search over the indexed environment.

Examples:

- “Where are my GGUF models?”
- “Show all Python installations.”
- “Which applications can edit video?”
- “What is using 40 GB in AI models?”
- “Do I still have anything from Ollama?”
- “Find duplicate models.”
- “What developer tools are installed?”
- “Can this PC run this model?”

Results should combine deterministic index facts with AI-generated explanation.

---

# 17. System Health

Create **Vertex System Health / Environment Doctor**.

Vertex should inspect more than currently installed applications.

It should identify stale or broken environmental state left behind after installation, upgrade, or removal.

Inspection domains may include:

- application install records,
- filesystem remnants,
- configuration files,
- environment variables,
- PATH,
- startup entries,
- services,
- scheduled tasks,
- ports,
- runtime references,
- model references,
- caches,
- registry entries on Windows,
- stale provider endpoints,
- invalid executable paths.

Example:

If Ollama has been uninstalled, Vertex may report:

- Ollama executable: not found
- Ollama service: not found
- model data: 6 GB remains
- configuration: remains
- PATH/reference: stale
- another application configuration still points to Ollama endpoint
- cloud provider: available

This is far more useful than “Ollama not found.”

---

# 18. Registry Safety

Windows Registry analysis is allowed as a diagnostic capability, but Vertex must **not become an aggressive registry cleaner**.

A registry entry is not safe to delete merely because Vertex cannot immediately identify it.

Before proposing cleanup, determine as much as possible about:

- ownership,
- referenced executable/component,
- shared component usage,
- COM relationships,
- service/driver relationships,
- installer metadata,
- current references,
- confidence that the entry is orphaned.

Classify findings by confidence and risk.

Examples:

- Verified orphan
- Likely orphan
- Unknown ownership
- Shared/system component
- Protected / do not modify

Automatic deletion of ambiguous or system-critical entries is prohibited.

Prefer diagnosis, explanation, and reversible remediation.

---

# 19. Health States

Use clear health states such as:

- Ready
- Warning
- Offline
- Misconfigured
- Missing dependency
- Conflict detected
- Orphan detected
- Repair available
- Unknown

The user should immediately understand whether something is usable.

---

# 20. Smart Fix & Guidance

Build **Smart Fix & Guidance**.

For every proposed repair:

1. Explain what Vertex found.
2. Separate fact from inference.
3. Explain why the issue matters.
4. Show the intended change.
5. Show risk level.
6. Offer a dry run where useful.
7. Obtain explicit user approval for meaningful changes.
8. Back up/recover where possible.
9. Execute through deterministic Core code.
10. Verify the result.
11. Record the action.
12. Offer rollback when supported.

The LLM recommends. The trusted execution layer validates and performs.

---

# 21. Vertex AI Error Intelligence

Replace opaque error-dialog UX with **Vertex AI Error Intelligence**.

Do not simply display canned technical messages.

When an error occurs, collect a bounded diagnostic context such as:

- operation that failed,
- raw error/error code,
- application/module,
- provider,
- model,
- runtime,
- relevant process/service status,
- dependency status,
- related configuration,
- relevant recent logs,
- Environment Explorer evidence,
- recent user action where available.

Then present:

### What happened
Plain-language explanation.

### What Vertex knows
Verified facts.

### Likely cause
AI inference, explicitly labeled as inference.

### What can be done
Safe options.

### Technical details
Raw error, logs, stack trace, IDs, and developer data.

Example:

> **Vertex could not reach the selected AI runtime.**
>
> Verified: Ollama is selected, but no Ollama service or executable is currently detected.
>
> Likely cause: an old provider configuration remained after Ollama was uninstalled.
>
> OpenAI connectivity is available.
>
> [Switch to OpenAI] [Repair Ollama configuration] [Technical details]

---

# 22. Predictive Error Prevention

Error Intelligence should eventually become proactive.

If Vertex detects a configuration that is likely to fail later, it may warn before failure.

Example:

> This configuration references an executable that no longer exists. The next model launch is likely to fail.

Never overwhelm the user with speculative warnings.

Use confidence thresholds and severity.

---

# 23. AI Device Manager Concept

The management UI should feel like an **AI-era Device Manager**.

At a glance, users should be able to understand:

- what AI systems exist,
- what models exist,
- where models are stored,
- what runtime launches them,
- which provider is active,
- whether each component is healthy,
- what applications/tools are available,
- what the computer is capable of,
- what is duplicated,
- what is broken,
- what can safely be repaired.

This is a core product identity.

---

# 24. Computer Capability Graph

Create an internal structured representation of the machine.

Think in terms of entities and relationships:

- Application
- Executable
- Runtime
- Provider
- Model
- Model file
- Service
- Process
- Port
- Dependency
- SDK
- Driver
- GPU
- Storage device
- Registry entry
- Environment variable
- Configuration
- Capability

Relationships may include:

- installed_at
- executes
- depends_on
- provides
- references
- stores
- launches
- listens_on
- compatible_with
- duplicates
- supersedes
- orphaned_from

This graph should make cross-domain diagnosis possible.

---

# 25. Memory

Vertex AI should be memory-centric, but memory must be intentional.

Separate:

- user/project memory,
- system/environment state,
- provider/model metadata,
- verified knowledge,
- temporary conversational context,
- logs/audit records.

Do not treat all of these as one undifferentiated vector store.

Use PostgreSQL-backed structured memory where appropriate and embeddings/RAG only where they add value.

---

# 26. Knowledge Core and Succession

Vertex must be designed so that the project can survive its original developer.

Create a **Vertex Knowledge Core** capable of preserving:

- product philosophy,
- architectural decisions,
- reasons behind decisions,
- coding conventions,
- security principles,
- rejected alternatives and why,
- component responsibilities,
- compatibility promises,
- build/deployment procedures,
- recovery procedures,
- terminology,
- roadmap intent,
- known technical debt.

The goal is not merely documentation.

The goal is **continuity of intent**.

A future maintainer should be able to ask:

> “Why was provider abstraction designed this way?”

and receive an answer grounded in versioned project records rather than AI invention.

---

# 27. Fact Preservation and Provenance

For long-term knowledge, distinguish source facts from interpretation.

Every important preserved fact should support provenance metadata where possible:

- source,
- author/system,
- timestamp,
- version,
- commit/release,
- confidence,
- immutable original text/hash where appropriate.

AI summaries must not silently overwrite source records.

Preserve originals and generate interpretations as derived layers.

This principle applies both to project history and, more broadly, to any future “AI storyteller/archivist” capability.

**Facts are records. Interpretations are views over records.**

---

# 28. Versioned Architecture Decisions

Use ADR-like records or an equivalent structured system.

Each major decision should capture:

- context,
- decision,
- alternatives,
- rationale,
- consequences,
- date/version,
- superseding decision if changed.

This gives future humans and AI maintainers a reliable development lineage.

---

# 29. UI / UX Principles

Take inspiration from tools such as LM Studio for clarity and discoverability, but do not imitate its visual design.

Vertex must have its own consistent design language.

UX priorities:

- current model visible,
- current provider visible,
- health visible,
- storage visible,
- clear difference between Model / Provider / Runtime / API,
- progressive disclosure,
- novice-friendly wording,
- developer details available on demand,
- dangerous actions visually distinct,
- no unexplained error codes as the primary message,
- no terminal requirement for ordinary operations.

The user should rarely need to leave Vertex to manage the AI environment.

---

# 30. Security Model

Apply least privilege.

Rules:

- inspection should not require elevation unless genuinely necessary,
- request elevation only for the specific privileged action,
- never allow arbitrary AI-generated shell commands to execute without validation,
- use allowlisted/typed operations for sensitive mutations,
- redact secrets from logs,
- record administrative changes,
- sandbox or constrain external tooling where practical,
- distinguish read-only diagnosis from mutation permissions.

---

# 31. Performance

The product must remain responsive on large developer/creator machines.

Requirements:

- background jobs,
- bounded concurrency,
- cancellable operations,
- incremental indexing,
- cached metadata,
- debounced filesystem events,
- lazy hashing,
- priority scheduling,
- UI progress reporting,
- avoid blocking the UI thread,
- avoid loading huge logs/files into the LLM unnecessarily.

AI must not sit in the hot path for operations that can be completed deterministically.

---

# 32. Offline and Degraded Operation

Vertex should degrade gracefully.

If cloud providers are unavailable:

- local models should remain usable where available,
- environment inspection should remain usable,
- deterministic diagnostics should remain usable,
- cached metadata should remain available.

If the configured provider disappears:

- do not enter an endless failure loop,
- identify the missing provider/runtime,
- explain the situation,
- offer available alternatives.

---

# 33. Extensibility

Design extension points for:

- providers,
- runtimes,
- model formats,
- scanners,
- capability classifiers,
- health checks,
- repair actions,
- application integrations,
- Vertex products.

Avoid giant switch statements tied to product names.

Prefer registries, adapters, typed capabilities, schemas, and versioned interfaces.

---

# 34. Integration with the Vertex Ecosystem

Vertex AI is a shared intelligence/control layer for the wider Vertex ecosystem.

Maintain compatibility with the broader direction:

> **Think -> Design -> Build -> Run -> Publish -> Sell -> Operate**

Future Vertex applications should be able to ask Vertex AI questions such as:

- Which models are available?
- Which provider should handle this task?
- Is the local AI runtime healthy?
- Does this computer have a video editor?
- Can this machine run this model?
- Which database/runtime is available?
- What environment problem caused this failure?

Do not tightly couple Vertex AI to one downstream product.

---

# 35. Implementation Strategy

Do not attempt every feature simultaneously.

Build vertical slices that remain architecturally correct.

Recommended order:

## Phase 0 — Foundations
- repository structure
- Rust Core
- Tauri/Vue shell
- typed IPC/API contracts
- configuration
- secure secret abstraction
- logging/audit foundations

## Phase 1 — Provider + Model Minimum
- provider interface
- one cloud provider
- one local provider/runtime
- health checks
- model registry
- Manual routing
- AI Test Console

## Phase 2 — Environment Explorer
- filesystem/application discovery
- runtime detection
- developer-tool detection
- persistent index
- fast search
- storage inventory

## Phase 3 — Model Storage Intelligence
- model fingerprints
- duplicate detection
- storage roots
- compatibility classification
- migration dry run
- safe migration

## Phase 4 — System Health
- PATH/environment diagnostics
- stale endpoint detection
- service/startup/task inspection
- Windows registry read-only diagnostics
- orphan confidence model

## Phase 5 — Error Intelligence
- structured error envelope
- diagnostic context collection
- fact/inference separation
- AI explanation
- guided repair plans

## Phase 6 — Creator Capability Graph
- application classification
- capability mapping
- semantic “what can this PC do?” queries
- tool-selection recommendations

## Phase 7 — Smart Fix
- typed repair actions
- preview
- permission/elevation boundaries
- backup/rollback
- verification

## Phase 8 — Knowledge Core
- ADR ingestion
- architecture knowledge
- provenance
- versioned project philosophy
- maintainer Q&A grounded in project records

## Phase 9 — Auto/Council Routing and Advanced Orchestration
- routing policies
- task capability matching
- cost/privacy/performance policies
- multi-model workflows

---

# 36. Initial Data Contracts

Define stable typed entities early.

Suggested conceptual entities:

```text
Provider
ProviderCapability
Model
ModelArtifact
Runtime
Application
ApplicationCapability
Dependency
EnvironmentFinding
HealthCheck
RepairPlan
RepairAction
StorageRoot
MigrationJob
SystemAsset
SystemRelationship
ErrorEvent
DiagnosticContext
KnowledgeRecord
ArchitectureDecision
AuditEvent
```

Each entity should have a stable ID and versionable schema.

Do not let UI-specific structures become the canonical domain model.

---

# 37. Error Contract

All subsystems should return a structured error envelope rather than arbitrary strings.

Conceptually:

```text
error_id
timestamp
component
operation
severity
machine_readable_code
human_fallback_message
technical_message
causes[]
evidence_refs[]
suggested_check_ids[]
recoverable
retryable
```

Error Intelligence can enrich this structure, but raw evidence must remain intact.

---

# 38. Repair Contract

Repairs must be typed plans, not arbitrary AI scripts.

Conceptually:

```text
repair_plan_id
finding_ids[]
summary
risk
requires_elevation
requires_restart
reversible
backup_strategy
actions[]
verification_steps[]
rollback_actions[]
```

AI may construct or select a candidate plan.

Core validates it against supported operations before execution.

---

# 39. Non-Goals / Prohibitions

Do not:

- turn Vertex AI into only a chat UI,
- hard-wire the product to Ollama, LM Studio, OpenAI, or any single provider,
- silently delete registry entries,
- silently delete model files,
- silently relocate user data,
- execute arbitrary LLM-generated shell commands as administrator,
- claim two runtimes can share a model file unless verified,
- rescan entire disks unnecessarily,
- store API keys in plaintext,
- hide raw technical details from advanced users,
- present AI guesses as facts,
- let AI-generated summaries overwrite historical source records,
- make the UI responsible for OS discovery logic,
- require command-line use for normal user workflows.

---

# 40. Definition of Success

Vertex AI succeeds when a user can install/open it and quickly understand:

1. What AI models do I have?
2. Where are they?
3. Am I storing duplicates?
4. What provider/runtime is being used?
5. Is it working?
6. If not, why?
7. Can Vertex safely fix it?
8. What developer and creator tools are already installed?
9. What can this computer do with those tools?
10. What dependencies are missing?
11. Is stale configuration from removed software causing problems?
12. Can I move large AI assets to another drive safely?
13. Can another Vertex product reuse this intelligence?
14. Can a future maintainer understand why Vertex was built this way?

The ideal experience is:

> **Vertex understands the machine so the user does not have to become a systems engineer just to use AI.**

---

# 41. Codex Execution Directive

When implementing this specification:

1. Inspect the existing repository before changing architecture.
2. Preserve working functionality unless the specification explicitly replaces it.
3. Identify conflicts between current code and this master design.
4. Propose the smallest coherent implementation slice.
5. Keep domain logic out of the UI.
6. Prefer typed Rust interfaces and stable schemas.
7. Add tests for discovery, classification, migration, health checks, and repair validation.
8. Treat OS mutations as security-sensitive.
9. Keep Windows-specific implementation behind platform abstractions.
10. Document architectural decisions as they are made.
11. Update the Knowledge Core/ADR records when architectural behavior changes.
12. Never invent successful system state: verify after every mutation.
13. Favor incremental, compilable, testable commits over a massive rewrite.

Before implementing a feature, ask internally:

> Is this deterministic fact collection, AI interpretation, or privileged mutation?

Then place it in the correct layer.

---

# 42. Core Design Maxim

Use this as the final architectural test:

> **Observe deterministically. Understand intelligently. Explain humanly. Act safely. Preserve the reason why.**

That is the foundation of Vertex AI.
