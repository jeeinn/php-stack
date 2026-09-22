import { describe, it, expect, beforeEach } from 'vitest'
import {
  showToast,
  getToasts,
  removeToast,
  addLog,
  getLogs,
  clearLogs,
  formatLogLine,
  setMinLogLevel,
  getMinLogLevel,
  visibleLogs,
  LOG_LEVEL_STORAGE_KEY,
} from '../useToast'

describe('useToast toasts', () => {
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
})

describe('useToast logs', () => {
  beforeEach(() => {
    clearLogs()
    setMinLogLevel('info')
    localStorage.removeItem(LOG_LEVEL_STORAGE_KEY)
  })

  it('adds log messages with default info level', () => {
    addLog('Test log message')
    const logs = getLogs()
    expect(logs.value.length).toBeGreaterThan(0)
    const last = logs.value[logs.value.length - 1]
    expect(last.message).toBe('Test log message')
    expect(last.level).toBe('info')
    expect(formatLogLine(last)).toContain('INFO Test log message')
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
    expect(logs().value[0].message).toBe('line-5')
    expect(logs().value[logs().value.length - 1].message).toBe(`line-${UI_LOG_LIMIT + 4}`)
  })

  it('filters visible logs by min level without dropping the buffer', () => {
    addLog('info line', 'info')
    addLog('warn line', 'warn')
    addLog('error line', 'error')
    expect(getLogs().value).toHaveLength(3)

    setMinLogLevel('warn')
    expect(getMinLogLevel().value).toBe('warn')
    expect(visibleLogs.value.map((e) => e.message)).toEqual(['warn line', 'error line'])

    setMinLogLevel('error')
    expect(visibleLogs.value.map((e) => e.message)).toEqual(['error line'])

    setMinLogLevel('info')
    expect(visibleLogs.value).toHaveLength(3)
  })

  it('persists min log level to localStorage', () => {
    setMinLogLevel('error')
    expect(localStorage.getItem(LOG_LEVEL_STORAGE_KEY)).toBe('error')
  })
})
