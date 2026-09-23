import { invokeCommand } from './client'
import type {
  BackupOptions,
  RestorePreview,
  RestoreResult,
  SitePathOverride,
} from '../types/env-config'

export function previewRestore(zipPath: string): Promise<RestorePreview> {
  return invokeCommand<RestorePreview>('preview_restore', { zipPath })
}

export function verifyBackup(zipPath: string): Promise<boolean> {
  return invokeCommand<boolean>('verify_backup', { zipPath })
}

export function executeRestore(
  zipPath: string,
  pathOverrides: SitePathOverride[],
): Promise<RestoreResult> {
  return invokeCommand<RestoreResult>('execute_restore', { zipPath, pathOverrides })
}

export function createBackup(
  savePath: string,
  options: BackupOptions,
): Promise<void> {
  return invokeCommand<void>('create_backup', { savePath, options })
}

export function getBackupOptions(): Promise<BackupOptions> {
  return invokeCommand<BackupOptions>('get_backup_options')
}

export function saveBackupOptions(options: BackupOptions): Promise<void> {
  return invokeCommand<void>('save_backup_options', { options })
}

export function convertToRelativePath(
  absolutePath: string,
  isDirectory: boolean,
): Promise<string> {
  return invokeCommand<string>('convert_to_relative_path', {
    absolutePath,
    isDirectory,
  })
}

export function normalizeMountPath(absolutePath: string): Promise<string> {
  return invokeCommand<string>('normalize_mount_path', { absolutePath })
}

export function relativePublicDir(mountPath: string, publicPath: string): Promise<string> {
  return invokeCommand<string>('relative_public_dir', { mountPath, publicPath })
}
