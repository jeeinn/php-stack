import { invoke as tauriInvoke } from '@tauri-apps/api/core'

/** 后端用此前缀传递可机读的端口冲突详情（见 start_environment） */
export const PORT_CONFLICT_PREFIX = 'PORT_CONFLICT:'

/** 就绪站点挂载目录缺失（见 start_environment / ensure_ready_site_mounts） */
export const SITE_MOUNT_PREFIX = 'SITE_MOUNT:'

/**
 * 把 Tauri / 未知错误规范成可读字符串，避免 `e as string` 变成 `[object Object]`。
 */
export function normalizeError(error: unknown): string {
  if (typeof error === 'string') return error
  if (error instanceof Error) return error.message
  if (error && typeof error === 'object') {
    const maybe = error as { message?: unknown }
    if (typeof maybe.message === 'string') return maybe.message
    try {
      return JSON.stringify(error)
    } catch {
      return String(error)
    }
  }
  return String(error)
}

export function isPortConflictError(error: unknown): boolean {
  return normalizeError(error).startsWith(PORT_CONFLICT_PREFIX)
}

export function isSiteMountError(error: unknown): boolean {
  return normalizeError(error).startsWith(SITE_MOUNT_PREFIX)
}

/** 剥离协议前缀，返回详情正文；无该前缀时原样返回 */
export function stripProtocolPrefix(message: string, prefix: string): string {
  return message.startsWith(prefix) ? message.slice(prefix.length) : message
}

/** 解析 `SITE_MOUNT:id (server): path; ...` 为站点 id 列表。 */
export function parseSiteMountErrorIds(error: unknown): string[] {
  const details = stripProtocolPrefix(normalizeError(error), SITE_MOUNT_PREFIX)
  return details
    .split(';')
    .map(part => part.trim())
    .filter(Boolean)
    .map(part => part.split(/\s|\(/)[0]?.trim() ?? '')
    .filter(Boolean)
}

/**
 * 类型安全的 Tauri invoke：失败时一律抛出 `string`（已 normalize）。
 */
export async function invokeCommand<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return args === undefined
      ? await tauriInvoke<T>(cmd)
      : await tauriInvoke<T>(cmd, args)
  } catch (e) {
    throw normalizeError(e)
  }
}
