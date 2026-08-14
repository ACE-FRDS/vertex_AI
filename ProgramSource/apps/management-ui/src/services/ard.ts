import { isDesktopRuntime } from './memory'

export type ArdBrain =
  | { kind: 'auto' }
  | { kind: 'model'; provider_id: string; model_id: string; runtime_id: string | null }

export type ToolCapability =
  | 'read_files' | 'write_files' | 'delete_files' | 'terminal'
  | 'git_read' | 'git_write' | 'network'

export interface ArdMember {
  id: string
  name: string
  role: string
  brain: ArdBrain
  permission: { allowed: ToolCapability[]; maximum_risk: 'LOW' | 'MEDIUM' | 'HIGH' | 'CRITICAL' }
  policy: { responsibilities: string[]; forbidden_actions: string[]; escalation_rules: string[] }
  workspace_id: string
  reports_to: string | null
  handoff_to: string | null
  enabled: boolean
}

export interface ArdTeam {
  id: string
  name: string
  workspace_id: string
  members: ArdMember[]
  created_at: string
  updated_at: string
}

export interface ArdWorkflowStage {
  id: string
  member_id: string
  objective: string
  on_success: string | null
  on_rework: string | null
  max_attempts: number
}

export interface ArdWorkflow {
  id: string
  team_id: string
  name: string
  entry_stage_id: string
  stages: ArdWorkflowStage[]
  created_at: string
}

export interface ArdSession {
  id: string
  team_id: string
  workflow_id: string
  workspace_id: string
  goal: string
  state: 'QUEUED' | 'RUNNING' | 'PAUSED' | 'WAITING_APPROVAL' | 'COMPLETED' | 'FAILED' | 'CANCELLED'
  current_stage_id: string | null
  handoffs: Array<{ id: string; from_member_id: string; to_member_id: string | null; decision: 'accepted' | 'rework' | 'blocked'; task_result: string; next_action: string; confidence: number; created_at: string }>
  interventions: Array<{ instruction: string; created_at: string; delivered_to: string[] }>
  activity: Array<{ sequence: number; occurred_at: string; member_id: string | null; kind: string; message: string }>
  model_rotations: Array<{ from: string | null; to: string | null; reused_loaded_model: boolean; router_required: boolean; occurred_at: string }>
  active_model: string | null
  created_at: string
  updated_at: string
  completed_at: string | null
}

async function invokeDesktop<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktopRuntime()) throw new Error('desktop_required')
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<T>(command, args)
}

export function createArdTeam(input: {
  name: string
  workspace_id: string
  members: Array<{
    name: string
    role: string
    brain: ArdBrain
    permission: { allowed: ToolCapability[]; maximum_risk: 'LOW' | 'MEDIUM' | 'HIGH' | 'CRITICAL' }
    responsibilities: string[]
    forbidden_actions: string[]
  }>
}) {
  return invokeDesktop<ArdTeam>('create_ard_team', { input })
}

export function listArdTeams() {
  return invokeDesktop<ArdTeam[]>('list_ard_teams')
}

export function createArdWorkflow(teamId: string, name: string) {
  return invokeDesktop<ArdWorkflow>('create_ard_workflow', { teamId, name })
}

export function listArdWorkflows(teamId: string) {
  return invokeDesktop<ArdWorkflow[]>('list_ard_workflows', { teamId })
}

export function startArdSession(workflowId: string, goal: string) {
  return invokeDesktop<ArdSession>('start_ard_session', { workflowId, goal })
}

export function listArdSessions(limit = 50) {
  return invokeDesktop<ArdSession[]>('list_ard_sessions', { limit })
}

export function pauseArdSession(sessionId: string) {
  return invokeDesktop<ArdSession>('pause_ard_session', { sessionId })
}

export function resumeArdSession(sessionId: string) {
  return invokeDesktop<ArdSession>('resume_ard_session', { sessionId })
}

export function cancelArdSession(sessionId: string) {
  return invokeDesktop<ArdSession>('cancel_ard_session', { sessionId })
}

export function interveneArdSession(sessionId: string, instruction: string) {
  return invokeDesktop<ArdSession>('intervene_ard_session', { sessionId, instruction })
}
