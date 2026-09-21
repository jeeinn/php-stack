// 容器状态契约：与 Rust `docker::manager::ContainerState` 一一对应。
// 后端用 `#[serde(rename_all = "lowercase")]` 序列化，前端按同样的字面量解析。
// 此前两端靠 `format!("{:?}")` → `includes('running')` 猜测，Docker/bollard 升级即碎。
export type ContainerState =
  | 'created'
  | 'running'
  | 'paused'
  | 'restarting'
  | 'exited'
  | 'removing'
  | 'dead'
  | 'unknown';

export interface Container {
  id: string;
  name: string;
  image: string;
  status: string;
  state: ContainerState;
  ports: number[];
}

/** 仅 running 视为运行中；restarting 尚未真正提供服务，不算。 */
export function isContainerRunning(state: ContainerState): boolean {
  return state === 'running';
}

/**
 * 兜底解析：后端枚举升级、新增变体或字段异常时，未知值一律归为 'unknown'，
 * 不让反序列化把整个容器列表打崩。
 */
export function parseContainerState(raw: unknown): ContainerState {
  const known: ContainerState[] = [
    'created',
    'running',
    'paused',
    'restarting',
    'exited',
    'removing',
    'dead',
  ];
  const value = String(raw ?? '').toLowerCase();
  return (known as string[]).includes(value) ? (value as ContainerState) : 'unknown';
}
