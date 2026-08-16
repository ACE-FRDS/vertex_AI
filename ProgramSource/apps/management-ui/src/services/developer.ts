import { isDesktopRuntime } from './memory'

export type DeveloperMode = 'ASK' | 'READ_ONLY' | 'EDIT' | 'EXECUTE' | 'AUTO'
export type DeveloperTaskState =
  | 'QUEUED' | 'ANALYZING' | 'PLANNING' | 'IMPLEMENTING' | 'BUILDING'
  | 'TESTING' | 'FIXING' | 'REVIEWING' | 'WAITING_APPROVAL'
  | 'COMPLETED' | 'FAILED' | 'CANCELLED'
export type RiskLevel = 'LOW' | 'MEDIUM' | 'HIGH' | 'CRITICAL'

export interface DeveloperWorkspace {
  id: string
  name: string
  root: string
  git_enabled: boolean
  branch: string | null
  registered_at: string
  last_opened_at: string
}

export interface DeveloperPlanStep {
  id: number
  description: string
  state: 'pending' | 'in_progress' | 'completed' | 'failed'
}

export interface DeveloperPlanRevision {
  version: number
  reason: string
  steps: DeveloperPlanStep[]
  created_at: string
}

export interface DeveloperActivity {
  sequence: number
  occurred_at: string
  kind: string
  message: string
  detail: string | null
  risk: RiskLevel
}

export interface DeveloperFileChange {
  path: string
  kind: 'created' | 'modified' | 'deleted'
  additions: number
  deletions: number
  reason: string
}

export interface DeveloperCommand {
  id: string
  executable: string
  args: string[]
  working_directory: string
  process_id: number | null
  started_at: string
  finished_at: string | null
  timeout_ms: number
  exit_code: number | null
  stdout: string
  stderr: string
  status: 'RUNNING' | 'COMPLETED' | 'FAILED' | 'CANCELLED' | 'TIMEOUT'
}

export interface DeveloperTask {
  id: string
  workspace_id: string
  request: string
  mode: DeveloperMode
  model: string
  state: DeveloperTaskState
  risk: RiskLevel
  confidence: number
  confidence_reason: string
  plan_revisions: DeveloperPlanRevision[]
  activities: DeveloperActivity[]
  files_changed: DeveloperFileChange[]
  commands: DeveloperCommand[]
  errors: Array<{ error_type: string; code: string | null; file: string | null; line: number | null; message: string }>
  unified_diff: string
  result_summary: string | null
  knowledge_saved: boolean
  steps_completed: number
  tool_calls: number
  failed_attempts: number
  created_at: string
  updated_at: string
  completed_at: string | null
}

async function invokeDesktop<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktopRuntime()) throw new Error('desktop_required')
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<T>(command, args)
}

export function listDeveloperWorkspaces() {
  return invokeDesktop<DeveloperWorkspace[]>('list_developer_workspaces')
}

export function registerDeveloperWorkspace(name: string, root: string) {
  return invokeDesktop<DeveloperWorkspace>('register_developer_workspace', { name, root })
}

export function startDeveloperTask(input: {
  workspace_id: string
  request: string
  mode: DeveloperMode
  provider_id: string
  model_id: string
}) {
  return invokeDesktop<DeveloperTask>('start_developer_task', { input })
}

export function getDeveloperTask(taskId: string) {
  return invokeDesktop<DeveloperTask>('get_developer_task', { taskId })
}

export function listDeveloperTasks(limit = 50) {
  return invokeDesktop<DeveloperTask[]>('list_developer_tasks', { limit })
}

export function cancelDeveloperTask(taskId: string) {
  return invokeDesktop<boolean>('cancel_developer_task', { taskId })
}

export function rollbackDeveloperTask(taskId: string) {
  return invokeDesktop<DeveloperTask>('rollback_developer_task', { taskId })
}

export interface WorkspaceEntry { name: string; relative: string; kind: 'file' | 'directory' }
export function listWorkspaceDirectory(workspaceId: string, relative = '.') {
  return invokeDesktop<WorkspaceEntry[]>('list_workspace_directory', { workspaceId, relative })
}

export function readWorkspaceFile(workspaceId: string, relative: string) {
  return invokeDesktop<string>('read_workspace_file', { workspaceId, relative })
}

export function writeWorkspaceFile(workspaceId: string, relative: string, content: string) {
  return invokeDesktop<boolean>('write_workspace_file', { workspaceId, relative, content })
}
