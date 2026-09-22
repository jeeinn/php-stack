import { invokeCommand } from './client'

export interface SupportInfo {
  app_version: string
  os: string
  os_version: string
  arch: string
}

export function getSupportInfo(): Promise<SupportInfo> {
  return invokeCommand<SupportInfo>('get_support_info')
}
