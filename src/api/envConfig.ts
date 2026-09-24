import { invokeCommand } from './client'
import type {
  EnvConfig,
  ImagePresence,
  PullImageResultItem,
  VersionMappings,
} from '../types/env-config'

export function getVersionMappings(): Promise<VersionMappings> {
  return invokeCommand<VersionMappings>('get_version_mappings')
}

export function loadExistingConfig(): Promise<EnvConfig | null> {
  return invokeCommand<EnvConfig | null>('load_existing_config')
}

export function generateEnvConfig(config: EnvConfig): Promise<string> {
  return invokeCommand<string>('generate_env_config', { config })
}

export function previewCompose(config: EnvConfig): Promise<string> {
  return invokeCommand<string>('preview_compose', { config })
}

export function applyEnvConfig(
  config: EnvConfig,
  enableBackup: boolean,
): Promise<string[]> {
  return invokeCommand<string[]>('apply_env_config', { config, enableBackup })
}

export function checkServiceImagesPresence(
  config: EnvConfig,
): Promise<ImagePresence[]> {
  return invokeCommand<ImagePresence[]>('check_service_images_presence', {
    config,
  })
}

export function pullServiceImages(
  imageTags: string[],
): Promise<PullImageResultItem[]> {
  return invokeCommand<PullImageResultItem[]>('pull_service_images', {
    imageTags,
  })
}

export function saveUserOverride(
  serviceType: string,
  id: string,
  imageTag: string,
  description?: string,
): Promise<void> {
  return invokeCommand<void>('save_user_override', {
    serviceType,
    id,
    imageTag,
    description,
  })
}

export function addCustomVersion(params: {
  serviceType: string
  id: string
  displayName: string
  imageTag: string
  serviceDir: string
  defaultPort: number
  showPort: boolean
  eol: boolean
  description?: string
}): Promise<void> {
  return invokeCommand<void>('add_custom_version', {
    serviceType: params.serviceType,
    id: params.id,
    displayName: params.displayName,
    imageTag: params.imageTag,
    serviceDir: params.serviceDir,
    defaultPort: params.defaultPort,
    showPort: params.showPort,
    eol: params.eol,
    description: params.description,
  })
}

export function removeUserOverride(
  serviceType: string,
  id: string,
): Promise<void> {
  return invokeCommand<void>('remove_user_override', { serviceType, id })
}

export function resetAllOverrides(): Promise<void> {
  return invokeCommand<void>('reset_all_overrides')
}
