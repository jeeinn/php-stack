import { describe, it, expect } from 'vitest'
import { extractPortsFromConfig, formatContainerConflictMessage } from '../portChecker'
import type { EnvConfig } from '../../types/env-config'

describe('portChecker', () => {
  it('extracts ports from config', () => {
    const mockConfig: EnvConfig = {
      source_dir: '/test/path',
      timezone: 'Asia/Shanghai',
      services: [
        {
          service_type: 'PHP',
          version: '8.2',
          host_port: 9000,
          extensions: [],
        },
        {
          service_type: 'MySQL',
          version: '8.0',
          host_port: 3306,
          extensions: [],
        }
      ]
    }
    
    const ports = extractPortsFromConfig(mockConfig)
    expect(ports.size).toBe(2)
    expect(ports.has(9000)).toBe(true)
    expect(ports.has(3306)).toBe(true)
  })

  it('formats conflict message correctly', () => {
    const conflicts = [
      {
        port: 3306,
        service: 'mysql80',
        container_name: 'existing-mysql',
        container_image: 'mysql:8.0',
        container_id: 'abc123def456'
      }
    ]
    
    const message = formatContainerConflictMessage(conflicts)
    expect(message).toContain('发现 1 个端口冲突')
    expect(message).toContain('3306')
    expect(message).toContain('existing-mysql')
  })
})
