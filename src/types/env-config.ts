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
  /** 启用 Nginx 时的站点。空数组表示只有一条 SOURCE_DIR 挂载。 */
  sites?: SiteEntry[];
}

export interface SiteEntry {
  id: string;
  server_name: string;
  /** 挂进容器的项目根。PHP 能读到其中的依赖。 */
  host_path: string;
  env_key: string;
  container_path: string;
  nginx_service: string;
  php_service: string;
  /** 相对挂载目录的对外子目录，如 public、www。空表示挂载目录本身就是网站根。 */
  public_dir?: string;
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
  // 数据库/vhost 备份暂缓：涉及跨平台备份恢复、本地库容量未知、备份进度不可控
  // include_database: boolean;
  include_projects: boolean;
  project_patterns: string[];
  // include_vhosts: boolean;
  include_logs: boolean;
  /** 勾选包含项目文件时，要打包源码的站点 id */
  site_ids?: string[];
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
  sites?: ManifestSite[];
}

export interface ManifestSite {
  id: string;
  env_key: string;
  container_path: string;
  host_path: string;
  kind: 'workspace-relative' | 'absolute' | string;
  server_name: string;
  /** 相对挂载目录的对外子目录。旧备份没有该字段。 */
  public_dir?: string;
}

export interface SitePathOverride {
  env_key: string;
  host_path: string;
  skipped: boolean;
}

export interface PortConflict {
  service: string;
  port: number;
  suggested_port: number;
}

export interface RestorePreview {
  manifest: BackupManifest;
  file_count: number;
  /** 预览时检测到的宿主机端口冲突（提示改端口，不阻断恢复） */
  port_conflicts: PortConflict[];
}

export interface BackupProgress {
  step: string;
  percentage: number;
}

export interface RestoreProgress {
  step: string;
  percentage: number;
}

/// 后端 `RestoreResult` 序列化形态
///
/// 恢复命令始终返回该结构（U2）：
/// - success=true：全部成功
/// - success=false 且 restored_files 非空：部分失败
/// - success=false 且 restored_files 为空：致命失败（包打不开 / zip-slip 等）
/// 前端用结果面板展示明细与回滚包快捷入口，不只弹 toast。
export interface RestoreResult {
  success: boolean;
  restored_files: string[];
  errors: string[];
  /// 恢复前自动生成的回滚包绝对路径（R2）。无回滚包时后端省略该字段。
  rollback_path?: string | null;
}

// Docker 容器端口冲突信息
export interface ContainerPortConflict {
  port: number;
  service: string;
  container_name: string;
  container_image: string;
  container_id: string;
}

// ==================== Phase 3: 镜像探测 / 拉取 / 配置提取 ====================

/// 后端 `ImageStatus` 序列化形态：`{"status": "present|missing", "tag": "...", "size": "..."}`
export interface ImagePresence {
  status: 'present' | 'missing';
  tag: string;
  size?: string | null;
}

/// 后端 `ExtractOutcome` 序列化形态
export interface ExtractResult {
  outcome: 'extracted' | 'skippedexists' | 'failed';
  dest?: string;
  bytes?: number;
  reason?: string;
}

/// 后端 `PullImageResult` 序列化形态
export interface PullImageResultItem {
  tag: string;
  success: boolean;
  error?: string;
}
