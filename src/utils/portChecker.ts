import { invoke } from '@tauri-apps/api/core';
import type { ContainerPortConflict } from '../types/env-config';
import type { EnvConfig } from '../types/env-config';

interface Container {
  id: string;
  name: string;
  image: string;
  status: string;
  state: string;
  ports: number[];
}

/**
 * 从 EnvConfig 中提取所有需要检查的端口
 */
export function extractPortsFromConfig(config: EnvConfig): Map<number, string> {
  const portMap = new Map<number, string>();
  
  config.services.forEach(service => {
    const serviceName = `${service.service_type.toLowerCase()}${service.version.replace(/\./g, '')}`;
    portMap.set(service.host_port, serviceName);
  });
  
  return portMap;
}

/**
 * 检查配置中的端口是否与现有 Docker 容器冲突
 */
export async function checkContainerPortConflicts(
  config: EnvConfig
): Promise<{
  hasConflicts: boolean;
  conflicts: ContainerPortConflict[];
}> {
  try {
    // 1. 获取所有运行中的容器（包括非 ps- 前缀的）
    const containers = await invoke<Container[]>('list_all_running_containers');
    console.log('[DEBUG] 获取到的容器列表:', containers.map(c => ({ name: c.name, ports: c.ports })));
    
    // 2. 提取配置中需要的端口
    const requiredPorts = extractPortsFromConfig(config);
    
    // 3. 检查冲突
    const conflicts: ContainerPortConflict[] = [];
    
    for (const [port, service] of requiredPorts.entries()) {
      // 查找是否有容器占用了这个端口
      const occupyingContainer = containers.find(c => 
        c.ports.includes(port) && isRunning(c.state)
      );
      
      if (occupyingContainer) {
        conflicts.push({
          port,
          service,
          container_name: occupyingContainer.name,
          container_image: occupyingContainer.image,
          container_id: occupyingContainer.id.substring(0, 12), // 缩短 ID
        });
      }
    }
    
    return {
      hasConflicts: conflicts.length > 0,
      conflicts,
    };
  } catch (error) {
    console.error('检查容器端口冲突失败:', error);
    throw error;
  }
}

/**
 * 判断容器是否运行中
 */
function isRunning(state: string): boolean {
  const normalized = state.toLowerCase();
  return normalized.includes('running');
}

/**
 * 格式化容器端口冲突提示信息
 */
export function formatContainerConflictMessage(conflicts: ContainerPortConflict[]): string {
  if (conflicts.length === 0) {
    return '';
  }
  
  let message = `发现 ${conflicts.length} 个端口冲突：\n\n`;
  
  conflicts.forEach((conflict, index) => {
    message += `${index + 1}. 端口 ${conflict.port} (${conflict.service})\n`;
    message += `   被占用容器: ${conflict.container_name}\n`;
    message += `   镜像: ${conflict.container_image}\n`;
    message += `   容器ID: ${conflict.container_id}\n`;
    message += '\n';
  });
  
  message += '💡 解决方案：\n';
  message += '• 停止冲突容器: docker stop <容器名>\n';
  message += '• 或删除冲突容器: docker rm <容器名>\n';
  message += '• 或在环境配置中修改为其他端口\n';
  
  return message;
}

/**
 * 生成停止冲突容器的命令列表
 */
export function generateStopCommands(conflicts: ContainerPortConflict[]): string[] {
  return conflicts.map(conflict => `docker stop ${conflict.container_name}`);
}

/**
 * 生成删除冲突容器的命令列表
 */
export function generateRemoveCommands(conflicts: ContainerPortConflict[]): string[] {
  return conflicts.map(conflict => `docker rm -f ${conflict.container_name}`);
}
