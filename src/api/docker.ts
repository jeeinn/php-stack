import { invokeCommand } from './client'
import type { Container, DockerHostReport } from '../types/docker'

export type { DockerHostReport }

export function checkDocker(): Promise<void> {
  return invokeCommand<void>('check_docker')
}

/** 一次往返拿到引擎是否就绪，以及未就绪时该显示打开还是安装。 */
export function inspectDockerHost(): Promise<DockerHostReport> {
  return invokeCommand<DockerHostReport>('inspect_docker_host')
}

/** 启动本机已安装的 Docker Desktop。 */
export function openDockerDesktop(): Promise<void> {
  return invokeCommand<void>('open_docker_desktop')
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
