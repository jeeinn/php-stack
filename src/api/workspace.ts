import { invokeCommand } from './client'

/** 与后端 `get_workspace_info` 序列化形态对齐 */
export interface WorkspaceInfo {
  workspace_path: string
  effective_path: string
  using_fallback: boolean
  path_missing: boolean
  fallback_reason?: string | null
}

export function getWorkspaceInfo(): Promise<WorkspaceInfo | null> {
  return invokeCommand<WorkspaceInfo | null>('get_workspace_info')
}

export function setWorkspacePath(path: string): Promise<void> {
  return invokeCommand<void>('set_workspace_path', { path })
}

export function recreateWorkspaceDir(): Promise<void> {
  return invokeCommand<void>('recreate_workspace_dir')
}

export function checkConfigFilesExist(): Promise<string[]> {
  return invokeCommand<string[]>('check_config_files_exist')
}

export function exportLogsTo(dest: string): Promise<void> {
  return invokeCommand<void>('export_logs_to', { dest })
}
