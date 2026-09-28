import { describe, it, expect } from 'vitest'
import {
  resolveConnectInfo,
  formatConnectEndpoint,
} from '../connectHost'

describe('resolveConnectInfo', () => {
  it('single redis: prefers short name redis, also service_dir', () => {
    const info = resolveConnectInfo('redis62', 1, {
      container_port: 6379,
      short_name: 'redis',
    })
    expect(info.primary).toBe('redis')
    expect(info.alsoAvailable).toEqual(['redis62'])
    expect(info.containerPort).toBe(6379)
    expect(info.warnMulti).toBe(false)
    expect(info.shortName).toBe('redis')
  })

  it('multi redis: uses service_dir only, warns', () => {
    const info = resolveConnectInfo('redis62', 2, {
      container_port: 6379,
      short_name: 'redis',
    })
    expect(info.primary).toBe('redis62')
    expect(info.alsoAvailable).toEqual([])
    expect(info.warnMulti).toBe(true)
    expect(info.shortName).toBe('redis')
  })

  it('single mysql / nginx: short aliases from catalog', () => {
    expect(
      resolveConnectInfo('mysql80', 1, {
        container_port: 3306,
        short_name: 'mysql',
      }).primary,
    ).toBe('mysql')
    expect(
      resolveConnectInfo('nginx128', 1, {
        container_port: 80,
        short_name: 'nginx',
      }).primary,
    ).toBe('nginx')
    expect(
      resolveConnectInfo('mysql80', 1, {
        container_port: 3306,
        short_name: 'mysql',
      }).containerPort,
    ).toBe(3306)
    expect(
      resolveConnectInfo('nginx128', 1, {
        container_port: 80,
        short_name: 'nginx',
      }).containerPort,
    ).toBe(80)
  })

  it('php always uses service_dir; multi php warns', () => {
    const single = resolveConnectInfo('php82', 1, {
      container_port: 9000,
      short_name: null,
    })
    expect(single.primary).toBe('php82')
    expect(single.alsoAvailable).toEqual([])
    expect(single.shortName).toBeNull()
    expect(single.warnMulti).toBe(false)
    expect(single.containerPort).toBe(9000)

    const multi = resolveConnectInfo('php82', 2, {
      container_port: 9000,
      short_name: null,
    })
    expect(multi.primary).toBe('php82')
    expect(multi.warnMulti).toBe(true)
  })

  it('custom service with catalog connect', () => {
    const info = resolveConnectInfo('mongo7', 1, {
      container_port: 27017,
      short_name: 'mongo',
    })
    expect(info.primary).toBe('mongo')
    expect(info.alsoAvailable).toEqual(['mongo7'])
    expect(info.containerPort).toBe(27017)
  })

  it('formatConnectEndpoint joins host and container port', () => {
    expect(
      formatConnectEndpoint(
        resolveConnectInfo('redis62', 1, {
          container_port: 6379,
          short_name: 'redis',
        }),
      ),
    ).toBe('redis:6379')
    expect(
      formatConnectEndpoint(
        resolveConnectInfo('redis70', 2, {
          container_port: 6379,
          short_name: 'redis',
        }),
      ),
    ).toBe('redis70:6379')
  })
})
