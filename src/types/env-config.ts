// Service types matching Rust ServiceType enum
export type ServiceType = 'PHP' | 'MySQL' | 'Redis' | 'Nginx';

export interface ServiceEntry {
  service_type: ServiceType;
  version: string;
  host_port: number;
  extensions?: string[];
}

export interface EnvConfig {
  services: ServiceEntry[];
  source_dir: string;
  timezone: string;
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
  port_conflicts: PortConflict[];
  missing_images: string[];
}

export interface BackupProgress {
  step: string;
  percentage: number;
}

export interface RestoreProgress {
  step: string;
  percentage: number;
}
