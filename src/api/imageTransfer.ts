import { invokeCommand } from './client'
import type {
  ImageImportResult,
  ImageTransferProgress,
  WorkspaceImageEntry,
} from '../types/env-config'

export type { ImageImportResult, ImageTransferProgress, WorkspaceImageEntry }

export function listWorkspaceImages(): Promise<WorkspaceImageEntry[]> {
  return invokeCommand<WorkspaceImageEntry[]>('list_workspace_images')
}

export function exportWorkspaceImages(
  savePath: string,
  selectedRefs?: string[] | null,
): Promise<void> {
  return invokeCommand<void>('export_workspace_images', {
    savePath,
    selectedRefs: selectedRefs ?? null,
  })
}

export function importWorkspaceImages(
  tarPath: string,
): Promise<ImageImportResult> {
  return invokeCommand<ImageImportResult>('import_workspace_images', {
    tarPath,
  })
}
