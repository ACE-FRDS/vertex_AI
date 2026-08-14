CREATE SCHEMA IF NOT EXISTS vertex_ai_memory;

CREATE TABLE IF NOT EXISTS vertex_ai_memory.memories (
    memory_id uuid PRIMARY KEY,
    category text NOT NULL CHECK (category IN (
        'working', 'conversation', 'long_term', 'project', 'knowledge', 'decision',
        'preference', 'experience', 'success', 'failure', 'system', 'vxn_knowledge'
    )),
    scope_type text NOT NULL CHECK (scope_type IN (
        'system', 'organization', 'user', 'application', 'project', 'session'
    )),
    organization_id uuid,
    user_id uuid,
    application_id uuid,
    project_id uuid,
    session_id uuid,
    owner_id uuid,
    content text NOT NULL CHECK (length(btrim(content)) > 0),
    structured_content jsonb NOT NULL DEFAULT '{}'::jsonb,
    priority real NOT NULL DEFAULT 0.5 CHECK (priority >= 0 AND priority <= 1),
    confidence real NOT NULL DEFAULT 0.5 CHECK (confidence >= 0 AND confidence <= 1),
    source text NOT NULL CHECK (length(btrim(source)) > 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz,
    local_only boolean NOT NULL DEFAULT false,
    cloud_allowed boolean NOT NULL DEFAULT false,
    sensitive boolean NOT NULL DEFAULT false,
    share_scope text,
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    normalized_content text NOT NULL,
    version bigint NOT NULL DEFAULT 1 CHECK (version >= 1),
    search_vector tsvector GENERATED ALWAYS AS (to_tsvector('simple', content)) STORED,
    CONSTRAINT memories_scope_shape CHECK (
        (scope_type = 'system' AND organization_id IS NULL AND user_id IS NULL
            AND application_id IS NULL AND project_id IS NULL AND session_id IS NULL)
        OR (scope_type = 'organization' AND organization_id IS NOT NULL AND user_id IS NULL
            AND application_id IS NULL AND project_id IS NULL AND session_id IS NULL)
        OR (scope_type = 'user' AND user_id IS NOT NULL
            AND application_id IS NULL AND project_id IS NULL AND session_id IS NULL)
        OR (scope_type = 'application' AND application_id IS NOT NULL
            AND project_id IS NULL AND session_id IS NULL)
        OR (scope_type = 'project' AND application_id IS NOT NULL
            AND project_id IS NOT NULL AND session_id IS NULL)
        OR (scope_type = 'session' AND application_id IS NOT NULL AND session_id IS NOT NULL)
    ),
    CONSTRAINT memories_privacy_consistency CHECK (NOT (local_only AND cloud_allowed))
);

CREATE INDEX IF NOT EXISTS memories_scope_idx ON vertex_ai_memory.memories (
    scope_type, organization_id, user_id, application_id, project_id, session_id
);
CREATE UNIQUE INDEX IF NOT EXISTS memories_exact_dedup_idx
    ON vertex_ai_memory.memories (
        scope_type, organization_id, user_id, application_id,
        project_id, session_id, category, normalized_content
    ) NULLS NOT DISTINCT;
CREATE INDEX IF NOT EXISTS memories_search_idx
    ON vertex_ai_memory.memories USING gin (search_vector);
CREATE INDEX IF NOT EXISTS memories_expiry_idx
    ON vertex_ai_memory.memories (expires_at) WHERE expires_at IS NOT NULL;

CREATE TABLE IF NOT EXISTS vertex_ai_memory.memory_relations (
    source_memory_id uuid NOT NULL REFERENCES vertex_ai_memory.memories(memory_id) ON DELETE CASCADE,
    target_memory_id uuid NOT NULL REFERENCES vertex_ai_memory.memories(memory_id) ON DELETE CASCADE,
    relation_type text NOT NULL CHECK (length(btrim(relation_type)) > 0),
    weight real NOT NULL DEFAULT 1 CHECK (weight >= 0 AND weight <= 1),
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (source_memory_id, target_memory_id, relation_type),
    CHECK (source_memory_id <> target_memory_id)
);

-- pgvector-compatible boundary: storage can later migrate to vector without changing
-- MemoryRepository. No extension is required for local-first baseline operation.
CREATE TABLE IF NOT EXISTS vertex_ai_memory.memory_embeddings (
    memory_id uuid NOT NULL REFERENCES vertex_ai_memory.memories(memory_id) ON DELETE CASCADE,
    embedding_model text NOT NULL,
    dimensions integer NOT NULL CHECK (dimensions > 0),
    vector_data real[] NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (memory_id, embedding_model),
    CHECK (cardinality(vector_data) = dimensions)
);

CREATE TABLE IF NOT EXISTS vertex_ai_memory.memory_audit (
    audit_id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    memory_id uuid NOT NULL,
    action text NOT NULL CHECK (action IN ('created', 'updated', 'deleted', 'deduplicated')),
    actor_id uuid,
    occurred_at timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE vertex_ai_memory.memory_audit IS
    'Content-free mutation audit. Memory bodies and secrets must never be copied here.';
