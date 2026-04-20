// 版本信息接口
export interface VersionInfo {
  version: string;
  image: string;
  tag: string;
  full_name: string;
  eol: boolean;
  description?: string;
  has_user_override?: boolean; // 是否有用户自定义覆盖
}

// 版本映射数据
export interface VersionMappings {
  php: VersionInfo[];
  mysql: VersionInfo[];
  redis: VersionInfo[];
  nginx: VersionInfo[];
}

// 服务类型
export type ServiceType = 'php' | 'mysql' | 'redis' | 'nginx';
