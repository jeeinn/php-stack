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
    
    // Mock t 函数
    const mockT = (key: string, params?: Record<string, any>) => {
      if (key === 'envConfig.portChecker.header') {
        return `发现 ${params?.count} 个端口冲突：\n\n`;
      }
      if (key === 'envConfig.portChecker.item') {
        return `${params?.index}. 端口 ${params?.port} (${params?.service})\n   被占用容器: ${params?.container_name}\n   镜像: ${params?.container_image}\n   容器ID: ${params?.container_id}\n\n`;
      }
      if (key === 'envConfig.portChecker.solution') {
        return '💡 解决方案：\n• 停止冲突容器: docker stop <容器名>\n• 或删除冲突容器: docker rm <容器名>\n• 或在环境配置中修改为其他端口';
      }
      return key;
    };
    
    const message = formatContainerConflictMessage(conflicts, mockT)
    expect(message).toContain('发现 1 个端口冲突')
    expect(message).toContain('3306')
    expect(message).toContain('existing-mysql')
  })
})
