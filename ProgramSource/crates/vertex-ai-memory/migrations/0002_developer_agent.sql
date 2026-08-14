CREATE TABLE IF NOT EXISTS vertex_ai_memory.developer_workspaces (
    workspace_id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    root TEXT NOT NULL UNIQUE,
    git_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    branch TEXT,
    document JSONB NOT NULL,
    registered_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS vertex_ai_memory.development_tasks (
    task_id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES vertex_ai_memory.developer_workspaces(workspace_id),
    request TEXT NOT NULL,
    mode TEXT NOT NULL,
    state TEXT NOT NULL,
    model TEXT NOT NULL,
    risk TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 0,
    document JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS development_tasks_workspace_state_idx
    ON vertex_ai_memory.development_tasks (workspace_id, state, updated_at DESC);

CREATE TABLE IF NOT EXISTS vertex_ai_memory.development_events (
    event_id UUID PRIMARY KEY,
    task_id UUID NOT NULL REFERENCES vertex_ai_memory.development_tasks(task_id) ON DELETE CASCADE,
    sequence BIGINT NOT NULL,
    event_type TEXT NOT NULL CHECK (event_type IN (
        'development_task', 'development_plan', 'development_step', 'tool_execution',
        'file_change', 'command_execution', 'build_result', 'test_result',
        'error_event', 'fix_attempt', 'decision', 'model_execution',
        'review_result', 'final_result'
    )),
    payload JSONB NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (task_id, sequence)
);

CREATE INDEX IF NOT EXISTS development_events_task_time_idx
    ON vertex_ai_memory.development_events (task_id, occurred_at);
