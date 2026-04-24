import { describe, it, expect } from 'vitest'
import { showToast, getToasts, removeToast, addLog, getLogs } from '../useToast'

describe('useToast', () => {
  it('shows a toast message', () => {
    showToast('Test message', 'success')
    const toasts = getToasts()
    expect(toasts.value.length).toBeGreaterThan(0)
  })

  it('removes a toast by id', () => {
    const initialLength = getToasts().value.length
    showToast('Test message', 'info')
    const toasts = getToasts()
    if (toasts.value.length > 0) {
      const id = toasts.value[toasts.value.length - 1].id
      removeToast(id)
      expect(getToasts().value.length).toBe(initialLength)
    }
  })

  it('adds log messages', () => {
    addLog('Test log message')
    const logs = getLogs()
    expect(logs.value.length).toBeGreaterThan(0)
    expect(logs.value[logs.value.length - 1]).toContain('Test log message')
  })
})
