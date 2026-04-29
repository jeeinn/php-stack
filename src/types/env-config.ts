// Service types matching Rust ServiceType enum
export type ServiceType = 'PHP' | 'MySQL' | 'Redis' | 'Nginx';

// 小写服务类型（用于版本映射 key）
export type ServiceTypeLower = 'php' | 'mysql' | 'redis' | 'nginx';

// 版本映射数据（按服务类型分组）
export interface VersionMappings {
  php: VersionInfo[];
  mysql: VersionInfo[];
  redis: VersionInfo[];
  nginx: VersionInfo[];
}

export interface ServiceEntry {
  service_type: ServiceType;
  version: string;        // manifest ID，如 "php82"
  host_port: number;
  extensions?: string[];
}

export interface VersionInfo {
  id: string;              // manifest ID，如 "php82"
  display_name: string;    // 显示名称，如 "PHP 8.2"
  image_tag: string;       // 完整镜像名，如 "php:8.2-fpm"
  service_dir: string;     // 配置目录名，如 "php82"
  default_port: number;    // 默认端口
  show_port: boolean;      // 是否在 UI 显示端口配置
  eol: boolean;
  description?: string;
  has_user_override?: boolean;
}

export interface EnvConfig {
  services: ServiceEntry[];
  source_dir: string;
  timezone: string;
  mysql_root_password?: string;  // MySQL root密码（可选）
  puid?: number;                 // Host user ID for file permissions (Linux only)
  pgid?: number;                 // Host group ID for file permissions (Linux only)
}

export interface MirrorSourceOption {
  id: string;
  name: string;
  value: string;
  description: string;
}

export interface MergedMirrorCategory {
  category_id: string;
  options: MirrorSourceOption[];
  selected_id: string;
  current_value: string;
  has_user_override: boolean;
}

export interface MirrorPreset {
  name: string;
  docker_registry: string;
  apt: string;
  composer: string;
  npm: string;
}

export interface BackupOptions {
  include_database: boolean;
  include_projects: boolean;
  project_patterns: string[];
  include_vhosts: boolean;
  include_logs: boolean;
}

export interface ManifestService {
  name: string;
  image: string;
  version: string;
  ports: Record<number, number>;
}

export interface BackupManifest {
  version: string;
  timestamp: string;
  app_version: string;
  os_info: string;
  services: ManifestService[];
  options: BackupOptions;
  files: Record<string, string>;
  errors: string[];
}

export interface PortConflict {
  service: string;
  port: number;
  suggested_port: number;
}

export interface RestorePreview {
  manifest: BackupManifest;
  file_count: number;
}

export interface BackupProgress {
  step: string;
  percentage: number;
}

export interface RestoreProgress {
  step: string;
  percentage: number;
}

// Docker 容器端口冲突信息
export interface ContainerPortConflict {
  port: number;
  service: string;
  container_name: string;
  container_image: string;
  container_id: string;
}
