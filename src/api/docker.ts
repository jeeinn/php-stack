import { invokeCommand } from './client'
import type { Container } from '../types/docker'

export function checkDocker(): Promise<void> {
  return invokeCommand<void>('check_docker')
}

export function listContainers(): Promise<Container[]> {
  return invokeCommand<Container[]>('list_containers')
}

export function listAllRunningContainers(): Promise<Container[]> {
  return invokeCommand<Container[]>('list_all_running_containers')
}

export function startContainer(name: string): Promise<void> {
  return invokeCommand<void>('start_container', { name })
}

export function stopContainer(name: string): Promise<void> {
  return invokeCommand<void>('stop_container', { name })
}

export function startEnvironment(): Promise<string> {
  return invokeCommand<string>('start_environment')
}

export function stopEnvironment(): Promise<void> {
  return invokeCommand<void>('stop_environment')
}

export function restartEnvironment(): Promise<void> {
  return invokeCommand<void>('restart_environment')
}

export function openServiceConfig(serviceName: string): Promise<void> {
  return invokeCommand<void>('open_service_config', { serviceName })
}
