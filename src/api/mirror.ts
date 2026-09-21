import { invokeCommand } from './client'
import type { MergedMirrorCategory } from '../types/env-config'

export function getMergedMirrorList(): Promise<MergedMirrorCategory[]> {
  return invokeCommand<MergedMirrorCategory[]>('get_merged_mirror_list')
}

export function testMirror(url: string): Promise<boolean> {
  return invokeCommand<boolean>('test_mirror', { url })
}

export function saveSelectedMirrorOption(
  categoryId: string,
  optionId: string,
): Promise<void> {
  return invokeCommand<void>('save_selected_mirror_option', {
    categoryId,
    optionId,
  })
}

export function updateSingleMirror(
  category: string,
  source: string,
): Promise<void> {
  return invokeCommand<void>('update_single_mirror', { category, source })
}

export function saveUserMirrorCategory(
  categoryId: string,
  source: string,
  description?: string,
): Promise<void> {
  return invokeCommand<void>('save_user_mirror_category', {
    categoryId,
    source,
    description,
  })
}

export function removeUserMirrorCategory(categoryId: string): Promise<void> {
  return invokeCommand<void>('remove_user_mirror_category', { categoryId })
}

export function resetAllMirrorOverrides(): Promise<void> {
  return invokeCommand<void>('reset_all_mirror_overrides')
}
