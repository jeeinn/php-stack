import { describe, it, expect } from 'vitest'
import {
  normalizeError,
  isPortConflictError,
  isSiteMountError,
  parseSiteMountErrorIds,
  stripProtocolPrefix,
  PORT_CONFLICT_PREFIX,
  SITE_MOUNT_PREFIX,
} from '../client'

describe('normalizeError', () => {
  it('字符串原样返回', () => {
    expect(normalizeError('boom')).toBe('boom')
  })

  it('Error 取 message', () => {
    expect(normalizeError(new Error('failed'))).toBe('failed')
  })

  it('对象优先取 message 字段', () => {
    expect(normalizeError({ message: 'from-object' })).toBe('from-object')
  })

  it('无 message 的对象走 JSON', () => {
    expect(normalizeError({ code: 1 })).toBe('{"code":1}')
  })

  it('避免 [object Object]', () => {
    expect(normalizeError({})).not.toBe('[object Object]')
  })
})

describe('port conflict helpers', () => {
  it('识别 PORT_CONFLICT 前缀', () => {
    expect(isPortConflictError(`${PORT_CONFLICT_PREFIX}80 used`)).toBe(true)
    expect(isPortConflictError('other')).toBe(false)
  })

  it('剥离协议前缀', () => {
    expect(stripProtocolPrefix(`${PORT_CONFLICT_PREFIX}80; 3306`, PORT_CONFLICT_PREFIX)).toBe(
      '80; 3306',
    )
    expect(stripProtocolPrefix('plain', PORT_CONFLICT_PREFIX)).toBe('plain')
  })
})

describe('site mount helpers', () => {
  it('识别 SITE_MOUNT 前缀', () => {
    expect(isSiteMountError(`${SITE_MOUNT_PREFIX}demo2 (x): missing path`)).toBe(true)
    expect(isSiteMountError('other')).toBe(false)
  })

  it('解析站点 id 列表', () => {
    expect(
      parseSiteMountErrorIds(
        `${SITE_MOUNT_PREFIX}demo2 (shop.test): D:/old; demo3 (a.test): missing path`,
      ),
    ).toEqual(['demo2', 'demo3'])
  })
})
