/** 容器内互连用的服务类型（与 UI 分区一致） */
export type ConnectServiceKind = 'php' | 'mysql' | 'redis' | 'nginx'

export interface ConnectInfo {
  /** 推荐写入应用 / conf 的主机名 */
  primary: string
  /** 同样可解析的其它主机名（单实例时的 service_dir） */
  alsoAvailable: string[]
  /** 容器内端口（非宿主机映射端口） */
  containerPort: number
  /** 同类多实例：短名不可用，需用版本化主机名 */
  warnMulti: boolean
  /** 单实例时可用的短名；PHP 无短名 */
  shortName: string | null
}

const SHORT_NAMES: Record<Exclude<ConnectServiceKind, 'php'>, string> = {
  mysql: 'mysql',
  redis: 'redis',
  nginx: 'nginx',
}

const CONTAINER_PORTS: Record<ConnectServiceKind, number> = {
  php: 9000,
  mysql: 3306,
  redis: 6379,
  nginx: 80,
}

/**
 * 根据 service_dir 与同类实例数，解析容器内推荐连接主机名。
 * 与 compose 生成逻辑一致：MySQL/Redis/Nginx 仅在 count===1 时挂短别名。
 */
export function resolveConnectInfo(
  serviceDir: string,
  kind: ConnectServiceKind,
  count: number,
): ConnectInfo {
  const containerPort = CONTAINER_PORTS[kind]
  const warnMulti = count > 1

  if (kind === 'php') {
    return {
      primary: serviceDir,
      alsoAvailable: [],
      containerPort,
      warnMulti,
      shortName: null,
    }
  }

  const shortName = SHORT_NAMES[kind]
  if (count === 1) {
    return {
      primary: shortName,
      alsoAvailable: serviceDir === shortName ? [] : [serviceDir],
      containerPort,
      warnMulti: false,
      shortName,
    }
  }

  return {
    primary: serviceDir,
    alsoAvailable: [],
    containerPort,
    warnMulti: true,
    shortName,
  }
}

/** 推荐连接串，如 `redis:6379` */
export function formatConnectEndpoint(info: ConnectInfo): string {
  return `${info.primary}:${info.containerPort}`
}
