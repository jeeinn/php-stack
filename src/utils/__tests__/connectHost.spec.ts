import { describe, it, expect } from 'vitest'
import {
  resolveConnectInfo,
  formatConnectEndpoint,
} from '../connectHost'

describe('resolveConnectInfo', () => {
  it('single redis: prefers short name redis, also service_dir', () => {
    const info = resolveConnectInfo('redis62', 'redis', 1)
    expect(info.primary).toBe('redis')
    expect(info.alsoAvailable).toEqual(['redis62'])
    expect(info.containerPort).toBe(6379)
    expect(info.warnMulti).toBe(false)
    expect(info.shortName).toBe('redis')
  })

  it('multi redis: uses service_dir only, warns', () => {
    const info = resolveConnectInfo('redis62', 'redis', 2)
    expect(info.primary).toBe('redis62')
    expect(info.alsoAvailable).toEqual([])
    expect(info.warnMulti).toBe(true)
    expect(info.shortName).toBe('redis')
  })

  it('single mysql / nginx: short aliases', () => {
    expect(resolveConnectInfo('mysql80', 'mysql', 1).primary).toBe('mysql')
    expect(resolveConnectInfo('nginx128', 'nginx', 1).primary).toBe('nginx')
    expect(resolveConnectInfo('mysql80', 'mysql', 1).containerPort).toBe(3306)
    expect(resolveConnectInfo('nginx128', 'nginx', 1).containerPort).toBe(80)
  })

  it('php always uses service_dir; multi php warns', () => {
    const single = resolveConnectInfo('php82', 'php', 1)
    expect(single.primary).toBe('php82')
    expect(single.alsoAvailable).toEqual([])
    expect(single.shortName).toBeNull()
    expect(single.warnMulti).toBe(false)
    expect(single.containerPort).toBe(9000)

    const multi = resolveConnectInfo('php82', 'php', 2)
    expect(multi.primary).toBe('php82')
    expect(multi.warnMulti).toBe(true)
  })

  it('formatConnectEndpoint joins host and container port', () => {
    expect(formatConnectEndpoint(resolveConnectInfo('redis62', 'redis', 1))).toBe(
      'redis:6379',
    )
    expect(formatConnectEndpoint(resolveConnectInfo('redis70', 'redis', 2))).toBe(
      'redis70:6379',
    )
  })
})
