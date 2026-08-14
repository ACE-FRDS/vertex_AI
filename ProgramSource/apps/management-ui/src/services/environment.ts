export type AssetCategory =
  | 'ai'
  | 'developer'
  | 'creator'
  | 'runtime'
  | 'database'
  | 'server'
  | 'system'
  | 'hardware'
  | 'storage'

export type HealthState =
  | 'ready'
  | 'warning'
  | 'offline'
  | 'misconfigured'
  | 'missing_dependency'
  | 'conflict_detected'
  | 'orphan_detected'
  | 'repair_available'
  | 'unknown'

export interface EvidenceRef {
  source: string
  locator: string
  observed_at: string
  content_hash: string | null
  metadata: Record<string, unknown>
}

export interface SystemAsset {
  id: string
  name: string
  category: AssetCategory
  kind: string
  location: string | null
  version: string | null
  architecture: string | null
  health: HealthState
  capabilities: string[]
  evidence: EvidenceRef[]
  observed_at: string
  metadata: Record<string, unknown>
}

export interface SystemRelationship {
  source_id: string
  target_id: string
  kind: string
  verified: boolean
}

export interface EnvironmentSnapshot {
  scanned_at: string
  roots_scanned: string[]
  assets: SystemAsset[]
  relationships: SystemRelationship[]
}

export interface EnvironmentDelta {
  added: string[]
  updated: string[]
  removed: string[]
  relationships_changed: boolean
}

export interface IndexedEnvironmentSnapshot {
  snapshot: EnvironmentSnapshot
  delta: EnvironmentDelta | null
}

export interface ErrorEnvelope {
  error_id: string
  timestamp: string
  component: string
  operation: string
  severity: string
  machine_readable_code: string
  human_fallback_message: string
  technical_message: string
  causes: string[]
  evidence_refs: string[]
  suggested_check_ids: string[]
  recoverable: boolean
  retryable: boolean
}

export type EnvironmentScanResult =
  | { kind: 'ready'; result: IndexedEnvironmentSnapshot }
  | { kind: 'desktop_required' }
  | { kind: 'error'; error: ErrorEnvelope }

function isTauriRuntime(): boolean {
  return '__TAURI_INTERNALS__' in window
}

export async function scanEnvironment(): Promise<EnvironmentScanResult> {
  if (!isTauriRuntime()) return { kind: 'desktop_required' }
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const result = await invoke<IndexedEnvironmentSnapshot>('scan_environment')
    return { kind: 'ready', result }
  } catch (error) {
    return { kind: 'error', error: normalizeError(error) }
  }
}

export async function getCachedEnvironment(): Promise<EnvironmentSnapshot | null> {
  if (!isTauriRuntime()) return null
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<EnvironmentSnapshot | null>('get_environment_snapshot')
}

function normalizeError(error: unknown): ErrorEnvelope {
  if (typeof error === 'object' && error !== null && 'machine_readable_code' in error) {
    return error as ErrorEnvelope
  }
  return {
    error_id: 'ui:unknown',
    timestamp: new Date().toISOString(),
    component: 'management-ui',
    operation: 'scan_environment',
    severity: 'error',
    machine_readable_code: 'ipc_error',
    human_fallback_message: '環境情報を取得できませんでした。',
    technical_message: error instanceof Error ? error.message : String(error),
    causes: [],
    evidence_refs: [],
    suggested_check_ids: [],
    recoverable: true,
    retryable: true,
  }
}
