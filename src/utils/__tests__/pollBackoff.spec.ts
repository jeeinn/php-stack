import { describe, it, expect } from 'vitest'
import { nextPollDelay, POLL_INTERVAL_MS } from '../pollBackoff'

describe('轮询退避', () => {
  it('正常情况与短暂抖动都保持 5 秒', () => {
    expect(nextPollDelay(0)).toBe(POLL_INTERVAL_MS)
    expect(nextPollDelay(1)).toBe(POLL_INTERVAL_MS)
    expect(nextPollDelay(2)).toBe(POLL_INTERVAL_MS)
  })

  it('连续失败 3 次起退避到 15 秒', () => {
    expect(nextPollDelay(3)).toBe(15000)
    expect(nextPollDelay(5)).toBe(15000)
  })

  it('连续失败 6 次起退避到 30 秒', () => {
    expect(nextPollDelay(6)).toBe(30000)
    expect(nextPollDelay(100)).toBe(30000)
  })

  it('间隔随失败次数单调不减', () => {
    let previous = 0
    for (let failures = 0; failures <= 20; failures++) {
      const delay = nextPollDelay(failures)
      expect(delay).toBeGreaterThanOrEqual(previous)
      previous = delay
    }
  })
})
