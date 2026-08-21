export type ModelCapability =
  | 'coding'
  | 'reasoning'
  | 'review'
  | 'general'
  | 'tool_use'
  | 'structured_output'
  | 'long_context'

export interface RuntimeCompatibility {
  runtime_id: string
  state: 'available' | 'compatible' | 'planned' | 'unsupported' | 'unknown'
  reason: string
}

export interface ModelRecord {
  id: string
  display_name: string
  family: string | null
  format: string | null
  quantization: string | null
  parameter_size: string | null
  file_size: number | null
  storage_location_id: string | null
  storage_path: string | null
  runtime_compatibility: RuntimeCompatibility[]
  capabilities: ModelCapability[]
  context_length: number | null
  local: boolean
  installed: boolean
  health: 'ready' | 'unavailable' | 'missing' | 'invalid' | 'unknown'
  trust: 'discovered' | 'registered' | 'verified' | 'trusted'
  source: 'local_storage' | 'ollama' | 'lm_studio' | 'remote'
  source_key: string
  created_at: string
  updated_at: string
}

export interface ModelStorageLocation {
  id: string
  display_name: string
  path: string
  is_default: boolean
  availability: 'available' | 'unavailable' | 'missing'
  writable: boolean
  total_space: number | null
  free_space: number | null
  created_at: string
  updated_at: string
}

export interface CompatibilityAssessment {
  model_id: string
  state: 'compatible' | 'compatible_with_offload' | 'resource_constrained' | 'unsupported' | 'unknown'
  reasons: string[]
}

export interface HardwareSnapshot {
  system_ram_total: number | null
  system_ram_available: number | null
  gpu_vram_total: number | null
  gpu_vram_available: number | null
  gpu_vram_in_use: number
  storage_locations: ModelStorageLocation[]
  observed_at: string
}

export interface ModelManagementSnapshot {
  models: ModelRecord[]
  storage_locations: ModelStorageLocation[]
  duplicates: Array<{ model_ids: string[]; evidence: string[] }>
  hardware: HardwareSnapshot
  compatibility: CompatibilityAssessment[]
  observed_at: string
}

function isDesktopRuntime(): boolean {
  return '__TAURI_INTERNALS__' in window
}

export async function getModelManagementSnapshot(): Promise<ModelManagementSnapshot | null> {
  if (!isDesktopRuntime()) return null
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<ModelManagementSnapshot>('get_model_management_snapshot')
}

export async function chooseModelStorageDirectory(): Promise<string | null> {
  if (!isDesktopRuntime()) return null
  const { open } = await import('@tauri-apps/plugin-dialog')
  const selected = await open({ directory: true, multiple: false, title: 'モデル保存先を選択' })
  return typeof selected === 'string' ? selected : null
}

export async function addModelStorageLocation(
  displayName: string,
  path: string,
): Promise<ModelStorageLocation> {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<ModelStorageLocation>('add_model_storage_location', {
    input: { display_name: displayName, path },
  })
}

export async function setDefaultModelStorage(storageId: string): Promise<ModelStorageLocation> {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<ModelStorageLocation>('set_default_model_storage', { storageId })
}

export async function scanModelStorage(storageId?: string): Promise<ModelManagementSnapshot> {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<ModelManagementSnapshot>('scan_model_storage', { storageId: storageId ?? null })
}
