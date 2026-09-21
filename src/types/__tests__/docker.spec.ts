import { describe, it, expect } from 'vitest'
import { isContainerRunning, parseContainerState } from '../docker'
import type { ContainerState } from '../docker'

describe('docker 容器状态契约', () => {
  it('仅 running 判定为运行中', () => {
    const running: ContainerState[] = ['running']
    const notRunning: ContainerState[] = [
      'created',
      'paused',
      'restarting',
      'exited',
      'removing',
      'dead',
      'unknown',
    ]

    for (const s of running) {
      expect(isContainerRunning(s)).toBe(true)
    }
    for (const s of notRunning) {
      expect(isContainerRunning(s)).toBe(false)
    }
  })

  it('后端枚举值全部能被识别，不落入 unknown', () => {
    const backendValues = [
      'created',
      'running',
      'paused',
      'restarting',
      'exited',
      'removing',
      'dead',
    ]
    for (const v of backendValues) {
      expect(parseContainerState(v)).toBe(v)
    }
  })

  it('未知或缺失状态降级为 unknown，而不是抛错', () => {
    expect(parseContainerState('SOMETHING_NEW')).toBe('unknown')
    expect(parseContainerState(undefined)).toBe('unknown')
    expect(parseContainerState(null)).toBe('unknown')
  })

  it('兼容历史遗留的 "Some(RUNNING)" 写法', () => {
    // 旧契约是 Rust Debug 输出；若仍有老版本后端在跑，不应把面板打崩
    expect(parseContainerState('Some(RUNNING)')).toBe('unknown')
    expect(isContainerRunning(parseContainerState('Some(RUNNING)'))).toBe(false)
  })
})
