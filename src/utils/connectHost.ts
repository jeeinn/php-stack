/** 容器内互连用的服务 kind（内置 + 自定义） */
export type ConnectServiceKind = string

export interface ConnectInfo {
  /** 推荐写入应用 / conf 的主机名 */
  primary: string
  /** 同样可解析的其它主机名（单实例时的 service_dir） */
  alsoAvailable: string[]
  /** 容器内端口（非宿主机映射端口） */
  containerPort: number
  /** 同类多实例：短名不可用，需用版本化主机名 */
  warnMulti: boolean
  /** 单实例时可用的短名；无短名时为 null（如 PHP） */
  shortName: string | null
}

/** 来自服务目录的连接信息（container_port / short_name） */
export interface CatalogConnect {
  container_port: number
  short_name?: string | null
}

/**
 * 根据 service_dir、同类实例数与 catalog 连接信息，解析容器内推荐连接主机名。
 * 有 short_name 且 count===1 时用短别名；否则用 service_dir。
 */
export function resolveConnectInfo(
  serviceDir: string,
  count: number,
  catalogConnect?: CatalogConnect,
): ConnectInfo {
  const containerPort = catalogConnect?.container_port ?? 0
  const shortName = catalogConnect?.short_name ?? null
  const warnMulti = count > 1

  // 无短名（如 PHP / 未配置 alias）：始终用 service_dir
  if (!shortName) {
    return {
      primary: serviceDir,
      alsoAvailable: [],
      containerPort,
      warnMulti,
      shortName: null,
    }
  }

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
