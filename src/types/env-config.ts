// Service types matching Rust ServiceType enum
export type ServiceType = 'PHP' | 'MySQL' | 'Redis' | 'Nginx';

export interface ServiceEntry {
  service_type: ServiceType;
  version: string;
  host_port: number;
  extensions?: string[];
}

export interface VersionInfo {
  version: string;        // 版本号（如 "7.2", "1.27"）
  tag: string;            // 完整标签（如 "7.2-alpine", "1.27-alpine"）
  full_name: string;      // 完整镜像名（如 "redis:7.2-alpine"）
  eol: boolean;           // 是否已停止维护
  description?: string;   // 版本描述
  has_user_override?: boolean; // 是否有用户自定义覆盖
}

export interface EnvConfig {
  services: ServiceEntry[];
  source_dir: string;
  timezone: string;
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
