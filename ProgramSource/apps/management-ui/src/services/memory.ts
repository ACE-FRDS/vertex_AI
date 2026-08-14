export type ManagedRuntimeState = 'INITIALIZING' | 'READY' | 'STOPPED' | 'DEGRADED' | 'ERROR' | 'REPAIR_REQUIRED'

export interface MemoryCoreStatus {
  id: string
  display_name: string
  state: ManagedRuntimeState
  version: string | null
  runtime_location: string
  data_location: string
  host: string | null
  port: number | null
  database: string | null
  schema_version: string | null
  database_size_bytes: number | null
  connection_count: number | null
  backup_state: string
  last_successful_start: string | null
  last_error: string | null
  observed_at: string
}

export interface MemoryRecord {
  memory_id: string
  category: string
  scope: { scope_type: string }
  content: string
  priority: number
  confidence: number
  source: string
  updated_at: string
  privacy: { local_only: boolean; cloud_allowed: boolean; sensitive: boolean }
  version: number
}

export interface RuntimeDiagnosis {
  code: string
  summary: string
  detail: string
  repairable: boolean
  destructive: boolean
}

export function isDesktopRuntime(): boolean {
  return '__TAURI_INTERNALS__' in window
}

async function invokeDesktop<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktopRuntime()) throw new Error('desktop_required')
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<T>(command, args)
}

export function getMemoryCoreStatus() { return invokeDesktop<MemoryCoreStatus>('get_memory_core_status') }
export function startMemoryCore() { return invokeDesktop<MemoryCoreStatus>('start_memory_core') }
export function stopMemoryCore() { return invokeDesktop<MemoryCoreStatus>('stop_memory_core') }
export function restartMemoryCore() { return invokeDesktop<MemoryCoreStatus>('restart_memory_core') }
export function diagnoseMemoryCore() { return invokeDesktop<RuntimeDiagnosis[]>('diagnose_memory_core') }
export function searchSystemMemories(query = '', limit = 50) { return invokeDesktop<MemoryRecord[]>('search_system_memories', { query, limit }) }
export function storeSystemMemory(content: string, category = 'knowledge') {
  return invokeDesktop<MemoryRecord>('store_system_memory', { input: { content, category, priority: 0.5, confidence: 1 } })
}
