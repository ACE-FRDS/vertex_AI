import type { ErrorEnvelope } from './environment'

export interface ProviderHealth {
  state: 'healthy' | 'degraded' | 'unavailable'
  message?: string
}

export interface ModelDescriptor {
  reference: { provider_id: string; model_id: string }
  display_name: string
  capabilities: {
    tools: boolean
    vision: boolean
    structured_output: boolean
    streaming: boolean
  }
  context_size: number | null
  local: boolean
  input_cost_per_million: number | null
  output_cost_per_million: number | null
  available: boolean
  metadata: Record<string, unknown>
}

export interface LocalProviderStatus {
  health: ProviderHealth
  models: ModelDescriptor[]
}

export interface InstalledLocalModel {
  reference: { provider_id: string; model_id: string }
  display_name: string
  size_bytes: number
  digest: string | null
  format: string | null
  family: string | null
  parameter_size: string | null
  quantization_level: string | null
  context_length: number | null
  modified_at: string | null
}

export interface LoadedLocalModel {
  reference: { provider_id: string; model_id: string }
  size_bytes: number
  size_vram_bytes: number
  context_length: number | null
  expires_at: string | null
}

export interface LocalRuntimeSnapshot {
  provider_id: string
  display_name: string
  endpoint: string
  health: 'ready' | 'warning' | 'offline' | 'misconfigured' | 'unknown'
  version: string | null
  executable_path: string | null
  model_storage_path: string | null
  storage_total_bytes: number | null
  storage_available_bytes: number | null
  installed_models: InstalledLocalModel[]
  loaded_models: LoadedLocalModel[]
  checked_at: string
}

export type ModelDownloadState =
  | 'queued'
  | 'running'
  | 'cancelling'
  | 'succeeded'
  | 'failed'
  | 'cancelled'
  | 'interrupted'

export interface ModelDownloadJob {
  id: string
  provider_id: string
  model_id: string
  state: ModelDownloadState
  status: string
  completed_bytes: number
  total_bytes: number | null
  error_message: string | null
  created_at: string
  updated_at: string
}

export interface AiEnvironmentSummary {
  runtimes: LocalRuntimeSnapshot[]
  runtime_count: number
  ready_runtime_count: number
  installed_model_count: number
  loaded_model_count: number
  total_model_bytes: number
  total_vram_bytes: number
  local_inference_ready: boolean
  observed_at: string
}

export interface LocalGenerationInput {
  model_id: string
  prompt: string
  temperature?: number
  max_output_tokens?: number
}

export interface GenerationResponse {
  response_id: string
  model: { provider_id: string; model_id: string }
  text: string
  usage: { input_tokens: number; output_tokens: number }
  finish_reason: string | null
  created_at: string
}

export interface AuditEvent {
  id: string
  occurred_at: string
  actor: string
  operation: string
  target_ids: string[]
  outcome: 'succeeded' | 'failed' | 'rejected' | 'rolled_back'
  elevated: boolean
  details: Record<string, unknown>
}

export type LocalProviderResult =
  | { kind: 'ready'; status: LocalProviderStatus }
  | { kind: 'desktop_required' }
  | { kind: 'error'; error: ErrorEnvelope }

export type LocalGenerationResult =
  | { kind: 'ready'; response: GenerationResponse }
  | { kind: 'desktop_required' }
  | { kind: 'error'; error: ErrorEnvelope }

function isTauriRuntime(): boolean {
  return '__TAURI_INTERNALS__' in window
}

export async function getLocalProviderStatus(): Promise<LocalProviderResult> {
  if (!isTauriRuntime()) return { kind: 'desktop_required' }
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const status = await invoke<LocalProviderStatus>('get_local_provider_status')
    return { kind: 'ready', status }
  } catch (error) {
    return { kind: 'error', error: normalizeError(error, 'get_local_provider_status') }
  }
}

export async function getAiEnvironment(): Promise<AiEnvironmentSummary | null> {
  if (!isTauriRuntime()) return null
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<AiEnvironmentSummary>('get_ai_environment')
}

export async function unloadLocalModel(
  providerId: string,
  modelId: string,
): Promise<LocalRuntimeSnapshot> {
  if (!isTauriRuntime()) throw new Error('desktop_required')
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<LocalRuntimeSnapshot>('unload_local_model', {
    providerId,
    modelId,
  })
}

export async function startModelDownload(
  providerId: string,
  modelId: string,
): Promise<ModelDownloadJob> {
  if (!isTauriRuntime()) throw new Error('desktop_required')
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<ModelDownloadJob>('start_model_download', { providerId, modelId })
}

export async function listModelDownloads(): Promise<ModelDownloadJob[]> {
  if (!isTauriRuntime()) return []
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<ModelDownloadJob[]>('list_model_downloads')
}

export async function cancelModelDownload(jobId: string): Promise<ModelDownloadJob> {
  if (!isTauriRuntime()) throw new Error('desktop_required')
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<ModelDownloadJob>('cancel_model_download', { jobId })
}

export async function generateLocal(input: LocalGenerationInput): Promise<LocalGenerationResult> {
  if (!isTauriRuntime()) return { kind: 'desktop_required' }
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const response = await invoke<GenerationResponse>('generate_local', { input })
    return { kind: 'ready', response }
  } catch (error) {
    return { kind: 'error', error: normalizeError(error, 'generate_local') }
  }
}

export async function getAuditEvents(limit = 100): Promise<AuditEvent[]> {
  if (!isTauriRuntime()) return []
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<AuditEvent[]>('get_audit_events', { limit })
}

function normalizeError(error: unknown, operation: string): ErrorEnvelope {
  if (typeof error === 'object' && error !== null && 'machine_readable_code' in error) {
    return error as ErrorEnvelope
  }
  return {
    error_id: 'ui:unknown',
    timestamp: new Date().toISOString(),
    component: 'management-ui',
    operation,
    severity: 'error',
    machine_readable_code: 'ipc_error',
    human_fallback_message: 'ローカルAIとの通信に失敗しました。',
    technical_message: error instanceof Error ? error.message : String(error),
    causes: [],
    evidence_refs: [],
    suggested_check_ids: [],
    recoverable: true,
    retryable: true,
  }
}
