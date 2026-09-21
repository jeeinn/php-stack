import { invokeCommand } from './client'
import type {
  BackupOptions,
  RestorePreview,
  RestoreResult,
} from '../types/env-config'

export function previewRestore(zipPath: string): Promise<RestorePreview> {
  return invokeCommand<RestorePreview>('preview_restore', { zipPath })
}

export function verifyBackup(zipPath: string): Promise<boolean> {
  return invokeCommand<boolean>('verify_backup', { zipPath })
}

export function executeRestore(zipPath: string): Promise<RestoreResult> {
  return invokeCommand<RestoreResult>('execute_restore', { zipPath })
}

export function createBackup(
  savePath: string,
  options: BackupOptions,
): Promise<void> {
  return invokeCommand<void>('create_backup', { savePath, options })
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
