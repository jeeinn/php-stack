/**
 * EnvConfigPage.applyFlow.spec.ts
 *
 * Phase 3 三段式 apply 流程的测试：
 *   ① 探测镜像存在性（check_service_images_presence）
 *   ② 缺失则弹 ImagePullConfirmModal 让用户确认
 *   ③ 确认后批量拉取 → 重新探测 → apply_env_config
 *
 * 之所以独立成文件：这三个步骤需要在同一个 mount 里有状态地推进，
 * 与 EnvConfigPage.spec.ts 的「渲染/数据加载」关注点不同，
 * 且各自需要不同的 invoke 返回，拆开避免互相污染 mock 实现。
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import EnvConfigPage from '../EnvConfigPage.vue'
import ImagePullConfirmModal from '../ImagePullConfirmModal.vue'
import { invoke } from '@tauri-apps/api/core'
import { showToast } from '../../composables/useToast'
import { showConfirm } from '../../composables/useConfirmDialog'
import type { ImagePresence, PullImageResultItem, VersionInfo } from '../../types/env-config'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-shell', () => ({ open: vi.fn() }))
vi.mock('../../composables/useToast', () => ({ showToast: vi.fn() }))
vi.mock('../../composables/useConfirmDialog', () => ({ showConfirm: vi.fn() }))

const phpVersions: VersionInfo[] = [
  { id: 'php82', display_name: 'PHP 8.2', image_tag: 'php:8.2-fpm', service_dir: 'php82', default_port: 9000, show_port: false, eol: false },
]
const mysqlVersions: VersionInfo[] = [
  { id: 'mysql80', display_name: 'MySQL 8.0', image_tag: 'mysql:8.0', service_dir: 'mysql80', default_port: 3306, show_port: true, eol: false },
]

/** 组件挂载期会调用的基础命令；测试只 override 与镜像/apply 相关的部分 */
const baseInvoke: Record<string, unknown> = {
  get_version_mappings: { php: phpVersions, mysql: mysqlVersions, redis: [], nginx: [] },
  load_existing_config: {
    services: [
      { service_type: 'PHP', version: 'php82', host_port: 9000 },
      { service_type: 'MySQL', version: 'mysql80', host_port: 3306 },
    ],
    source_dir: './www',
    timezone: 'Asia/Shanghai',
  },
  get_workspace_info: { workspace_path: '/test/workspace' },
  check_config_files_exist: [],
  check_service_images_presence: [],
  pull_service_images: [],
  apply_env_config: [],
}

function setupInvoke(overrides: Record<string, unknown> = {}) {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd in overrides) return overrides[cmd]
    return baseInvoke[cmd] ?? null
  })
}

/** 统计某个命令被调用的次数 */
function callsTo(cmd: string) {
  return vi.mocked(invoke).mock.calls.filter((c) => c[0] === cmd)
}

/** 取某个命令最后一次调用时的入参 */
function lastArgsOf(cmd: string): any {
  const matched = vi.mocked(invoke).mock.calls.filter((c) => c[0] === cmd)
  return matched.length ? matched[matched.length - 1][1] : undefined
}

async function mountPage() {
  const wrapper = mount(EnvConfigPage)
  await flushPromises()
  return wrapper
}

async function clickApply(wrapper: ReturnType<typeof mount>) {
  const btn = wrapper.findAll('button').find((b) => b.text() === '应用配置')
  expect(btn, '应能找到「应用配置」按钮').toBeTruthy()
  await btn!.trigger('click')
  await flushPromises()
  return wrapper
}

function pullModal(wrapper: ReturnType<typeof mount>) {
  return wrapper.findComponent(ImagePullConfirmModal)
}

const ALL_PRESENT: ImagePresence[] = [
  { status: 'present', tag: 'php:8.2-fpm', size: '500MB' },
  { status: 'present', tag: 'mysql:8.0', size: '1.08GB' },
]
const ONE_MISSING: ImagePresence[] = [
  { status: 'present', tag: 'php:8.2-fpm', size: '500MB' },
  { status: 'missing', tag: 'mysql:8.0' },
]

describe('EnvConfigPage — Phase 3 三段式 apply', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    // 默认不勾选备份；个别测试会覆盖
    vi.mocked(showConfirm).mockResolvedValue({ confirmed: true, checkboxValue: true } as any)
  })

  it('① 镜像全部存在时跳过拉取，直接 apply', async () => {
    setupInvoke({ check_service_images_presence: ALL_PRESENT })

    const wrapper = await mountPage()
    await clickApply(wrapper)

    expect(callsTo('check_service_images_presence').length).toBe(1)
    expect(callsTo('pull_service_images').length).toBe(0)
    expect(callsTo('apply_env_config').length).toBe(1)
    expect(pullModal(wrapper).props('open')).toBe(false)
  })

  it('② 存在缺失镜像时弹出确认框，且此时不 apply', async () => {
    setupInvoke({ check_service_images_presence: ONE_MISSING })

    const wrapper = await mountPage()
    await clickApply(wrapper)

    expect(pullModal(wrapper).props('open')).toBe(true)
    expect(pullModal(wrapper).props('presences')).toEqual(ONE_MISSING)
    expect(callsTo('pull_service_images').length).toBe(0)
    expect(callsTo('apply_env_config').length).toBe(0)
  })

  it('③ 用户取消拉取时不 apply，弹窗关闭', async () => {
    setupInvoke({ check_service_images_presence: ONE_MISSING })

    const wrapper = await mountPage()
    await clickApply(wrapper)
    expect(pullModal(wrapper).props('open')).toBe(true)

    pullModal(wrapper).vm.$emit('cancel')
    await flushPromises()

    expect(pullModal(wrapper).props('open')).toBe(false)
    expect(callsTo('pull_service_images').length).toBe(0)
    expect(callsTo('apply_env_config').length).toBe(0)
  })

  it('④ 确认拉取且全部成功 → 拉取后重新探测并 apply', async () => {
    const pulled: PullImageResultItem[] = [{ tag: 'mysql:8.0', success: true }]
    setupInvoke({
      check_service_images_presence: ONE_MISSING,
      pull_service_images: pulled,
    })

    const wrapper = await mountPage()
    await clickApply(wrapper)
    pullModal(wrapper).vm.$emit('confirm', ['mysql:8.0'])
    await flushPromises()

    const pullArgs = lastArgsOf('pull_service_images')
    expect(pullArgs.imageTags).toEqual(['mysql:8.0'])
    // 探测一次（apply 前） + 拉取后重新探测一次
    expect(callsTo('check_service_images_presence').length).toBe(2)
    expect(callsTo('apply_env_config').length).toBe(1)
    expect(vi.mocked(showToast)).toHaveBeenCalled()
  })

  it('⑤ 拉取部分失败仍继续 apply（后端 fallback 兜底）', async () => {
    const partial: PullImageResultItem[] = [
      { tag: 'mysql:8.0', success: true },
      { tag: 'redis:8.2-alpine', success: false, error: 'manifest unknown' },
    ]
    setupInvoke({
      check_service_images_presence: ONE_MISSING,
      pull_service_images: partial,
    })

    const wrapper = await mountPage()
    await clickApply(wrapper)
    pullModal(wrapper).vm.$emit('confirm', ['mysql:8.0', 'redis:8.2-alpine'])
    await flushPromises()

    // 关键语义：部分失败不阻断，apply 必须仍然发生
    expect(callsTo('apply_env_config').length).toBe(1)
    // 且给了 warning 级别的提示
    const warnCall = vi.mocked(showToast).mock.calls.find((c) => c[1] === 'warning')
    expect(warnCall, '部分失败应给出 warning 提示').toBeTruthy()
  })

  it('⑥ 拉取全部失败也继续 apply（不阻断用户）', async () => {
    const allFailed: PullImageResultItem[] = [
      { tag: 'mysql:8.0', success: false, error: 'network unreachable' },
    ]
    setupInvoke({
      check_service_images_presence: ONE_MISSING,
      pull_service_images: allFailed,
    })

    const wrapper = await mountPage()
    await clickApply(wrapper)
    pullModal(wrapper).vm.$emit('confirm', ['mysql:8.0'])
    await flushPromises()

    expect(callsTo('apply_env_config').length).toBe(1)
    const warnCall = vi.mocked(showToast).mock.calls.find((c) => c[1] === 'warning')
    expect(warnCall, '全部失败应给出 warning 提示').toBeTruthy()
  })

  it('⑦ 用户在覆盖确认里取消备份时，走拉取路径后仍应保持不备份', async () => {
    // 已存在配置文件 → 触发覆盖确认；用户确认覆盖但**不勾选**备份
    setupInvoke({
      check_config_files_exist: ['.env'],
      check_service_images_presence: ONE_MISSING,
      pull_service_images: [{ tag: 'mysql:8.0', success: true }],
    })
    vi.mocked(showConfirm).mockResolvedValue({ confirmed: true, checkboxValue: false } as any)

    const wrapper = await mountPage()
    await clickApply(wrapper)
    pullModal(wrapper).vm.$emit('confirm', ['mysql:8.0'])
    await flushPromises()

    expect(callsTo('apply_env_config').length).toBe(1)
    const args = lastArgsOf('apply_env_config')
    expect(args.enableBackup, '用户取消了备份，不应被静默改回 true').toBe(false)
  })

  it('⑦b 用户勾选备份时，走拉取路径应保持备份开启', async () => {
    setupInvoke({
      check_config_files_exist: ['.env'],
      check_service_images_presence: ONE_MISSING,
      pull_service_images: [{ tag: 'mysql:8.0', success: true }],
    })
    vi.mocked(showConfirm).mockResolvedValue({ confirmed: true, checkboxValue: true } as any)

    const wrapper = await mountPage()
    await clickApply(wrapper)
    pullModal(wrapper).vm.$emit('confirm', ['mysql:8.0'])
    await flushPromises()

    const args = lastArgsOf('apply_env_config')
    expect(args.enableBackup).toBe(true)
  })

  it('⑧ 探测接口抛错时不 apply，且提示错误', async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === 'check_service_images_presence') {
        throw new Error('docker daemon not running')
      }
      return baseInvoke[cmd] ?? null
    })

    const wrapper = await mountPage()
    await clickApply(wrapper)

    expect(callsTo('apply_env_config').length).toBe(0)
    const errCall = vi.mocked(showToast).mock.calls.find((c) => c[1] === 'error')
    expect(errCall, '探测失败应给出 error 提示').toBeTruthy()
  })
})
