import { describe, it, expect, beforeEach } from 'vitest'
import {
  pendingUpdateVersion,
  setPendingUpdateVersion,
  showUpdateBanner,
  dismissUpdateBanner,
} from '../useUpdater'

describe('useUpdater', () => {
  beforeEach(() => {
    setPendingUpdateVersion('')
  })

  it('shows banner when a pending version is set', () => {
    setPendingUpdateVersion('0.5.2')
    expect(pendingUpdateVersion.value).toBe('0.5.2')
    expect(showUpdateBanner.value).toBe(true)
  })

  it('hides banner after dismiss while keeping pending version', () => {
    setPendingUpdateVersion('0.5.2')
    dismissUpdateBanner()
    expect(showUpdateBanner.value).toBe(false)
    expect(pendingUpdateVersion.value).toBe('0.5.2')
  })

  it('shows banner again when a newer pending version arrives', () => {
    setPendingUpdateVersion('0.5.2')
    dismissUpdateBanner()
    setPendingUpdateVersion('0.5.3')
    expect(showUpdateBanner.value).toBe(true)
    expect(pendingUpdateVersion.value).toBe('0.5.3')
  })

  it('does not re-show banner when the same version is set again', () => {
    setPendingUpdateVersion('0.5.2')
    dismissUpdateBanner()
    setPendingUpdateVersion('0.5.2')
    expect(showUpdateBanner.value).toBe(false)
  })
})
