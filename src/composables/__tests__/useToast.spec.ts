import { describe, it, expect } from 'vitest'
import { showToast, getToasts, removeToast, addLog, getLogs, clearLogs } from '../useToast'

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

  it('clears all log messages', () => {
    addLog('to be cleared')
    expect(getLogs().value.length).toBeGreaterThan(0)
    clearLogs()
    expect(getLogs().value.length).toBe(0)
  })

  it('caps UI logs at UI_LOG_LIMIT and drops the oldest', async () => {
    const { UI_LOG_LIMIT, clearLogs: clear, addLog: push, getLogs: logs } = await import('../useToast')
    clear()
    for (let i = 0; i < UI_LOG_LIMIT + 5; i++) {
      push(`line-${i}`)
    }
    expect(logs().value.length).toBe(UI_LOG_LIMIT)
    expect(logs().value[0]).toContain('line-5')
    expect(logs().value[logs().value.length - 1]).toContain(`line-${UI_LOG_LIMIT + 4}`)
  })
})
