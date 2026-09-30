import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { invoke } from '@tauri-apps/api/core'
import { getToasts } from '../composables/useToast'
import App from '../App.vue'

const open = vi.fn(async () => {})

function readyDockerReport() {
  return { kind: 'ready', detail: '', install_url: '', can_open: false }
}

vi.mock('@tauri-apps/plugin-shell', () => ({
  open: (...args: unknown[]) => open(...args),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (command: string) => {
    switch (command) {
      case 'inspect_docker_host':
        return { kind: 'ready', detail: '', install_url: '', can_open: false }
      case 'list_containers':
        return []
      case 'get_workspace_info':
        return null
      case 'check_config_files_exist':
        return []
      default:
        return null
    }
  }),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
}))

vi.mock('@tauri-apps/api/app', () => ({
  getVersion: vi.fn(async () => '0.5.0'),
}))

vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({
  writeText: vi.fn(async () => {}),
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({
  save: vi.fn(async () => null),
  open: vi.fn(async () => null),
}))

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: vi.fn(async () => null),
}))

describe('App 仪表盘 Docker 可用性检查', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  // Feature: dashboard-docker-check, Property: 挂载后必须真实调用后端 inspect_docker_host，
  // 并在引擎就绪后继续拉取容器列表。
  // 回归背景：局部包装函数与同名导入的 API 函数互相遮蔽，函数体内的 await 调用的是
  // 它自己，形成同步无限递归并抛 RangeError、被 catch 当作「Docker 不可用」吞掉；
  // 探测与 list_containers 均不会执行，仪表盘恒显示不可用。
  it('挂载后真实调用 inspect_docker_host 并接着拉取容器列表', async () => {
    const wrapper = mount(App)
    await flushPromises()
    await flushPromises()

    const commands = vi.mocked(invoke).mock.calls.map(c => c[0])
    expect(commands).toContain('inspect_docker_host')
    expect(commands).toContain('list_containers')

    wrapper.unmount()
  })

  // Feature: docker-host-actions, Property: 未安装只给安装和重试；已安装未启动且能打开时只给打开和重试。
  it('按探测结果显示安装或打开，技术原文留在副文案', async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === 'inspect_docker_host') {
        return {
          kind: 'not_installed',
          detail: 'Docker service unavailable or not running: client error (Connect)',
          install_url: 'https://docs.docker.com/desktop/setup/install/windows-install/',
          can_open: false,
        }
      }
      return null
    })

    const wrapper = mount(App)
    await flushPromises()
    await flushPromises()

    const texts = () => wrapper.findAll('button').map((b) => b.text().trim())
    expect(texts()).toContain('安装')
    expect(texts()).not.toContain('打开')
    expect(texts()).toContain('重试')
    expect(wrapper.text()).toContain('未检测到 Docker')
    expect(wrapper.text()).toContain('client error (Connect)')

    const install = wrapper.findAll('button').find((b) => b.text().trim() === '安装')!
    await install.trigger('click')
    await flushPromises()
    expect(open).toHaveBeenCalledWith('https://docs.docker.com/desktop/setup/install/windows-install/')

    wrapper.unmount()

    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === 'inspect_docker_host') {
        return {
          kind: 'installed_stopped',
          detail: 'Docker service unavailable or not running: client error (Connect)',
          install_url: 'https://docs.docker.com/desktop/setup/install/windows-install/',
          can_open: true,
        }
      }
      if (command === 'open_docker_desktop') return undefined
      return null
    })

    const running = mount(App)
    await flushPromises()
    await flushPromises()
    const runningTexts = running.findAll('button').map((b) => b.text().trim())
    expect(runningTexts).toContain('打开')
    expect(runningTexts).not.toContain('安装')
    expect(running.text()).toContain('Docker 未启动')

    const openBtn = running.findAll('button').find((b) => b.text().trim() === '打开')!
    await openBtn.trigger('click')
    await flushPromises()
    const commands = vi.mocked(invoke).mock.calls.map((c) => c[0])
    expect(commands).toContain('open_docker_desktop')
    expect(getToasts().value.some((t) => t.message.includes('已尝试打开 Docker Desktop'))).toBe(true)

    running.unmount()
  })

  it('权限不足和自定义地址不提供打开或安装', async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === 'inspect_docker_host') {
        return {
          kind: 'permission_denied',
          detail: 'Permission denied (os error 13)',
          install_url: 'https://docs.docker.com/desktop/setup/install/linux/',
          can_open: false,
        }
      }
      return null
    })
    const wrapper = mount(App)
    await flushPromises()
    await flushPromises()
    const texts = wrapper.findAll('button').map((b) => b.text().trim())
    expect(texts).toContain('重试')
    expect(texts).not.toContain('打开')
    expect(texts).not.toContain('安装')
    expect(wrapper.text()).toContain('无权访问 Docker')
    wrapper.unmount()
  })
})

/** 找到容器卡片里的启停按钮。locale 跟随 jsdom 的 navigator.language（可能 zh 或 en），
 * 顶部「一键启动」/「Start All」文本不同，不会误中 */
function findCardButton(wrapper: ReturnType<typeof mount>, texts: string[]) {
  const all = wrapper.findAll('button').map((b) => b.text().trim())
  const btn = wrapper.findAll('button').find((b) => texts.includes(b.text().trim()))
  expect(btn, `应存在文本为 ${JSON.stringify(texts)} 的容器卡片按钮；现有按钮: ${JSON.stringify(all)}`).toBeTruthy()
  return btn!
}

describe('App 容器卡片启停', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  const mountedExitedApp = async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      switch (command) {
        case 'inspect_docker_host':
          return readyDockerReport()
        case 'list_containers':
          return [
            { id: 'c1', name: 'ps-mysql80', image: 'mysql:8.0', status: 'Exited (0)', state: 'exited', ports: [] },
          ]
        case 'get_workspace_info':
          return null
        case 'check_config_files_exist':
          return []
        default:
          return null
      }
    })
    const wrapper = mount(App)
    await flushPromises()
    await flushPromises()
    return wrapper
  }

  // Feature: dashboard-container-actions, Property: 容器启动失败必须弹 error toast。
  // 回归背景：startService 失败只写实时日志面板（对不主动看日志的用户不可见），
  // 端口被占用（docker 报 port is already allocated）时界面毫无反馈。
  it('容器启动失败时弹出 error toast，端口占用不再只进日志', async () => {
    const wrapper = await mountedExitedApp()
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === 'inspect_docker_host') return readyDockerReport()
      if (command === 'start_container') {
        throw 'Error response from daemon: driver failed programming external connectivity: Bind for 0.0.0.0:3306 failed: port is already allocated'
      }
      if (command === 'list_containers') {
        return [{ id: 'c1', name: 'ps-mysql80', image: 'mysql:8.0', status: 'Exited (0)', state: 'exited', ports: [] }]
      }
      return null
    })

    const toastsBefore = getToasts().value.length
    await findCardButton(wrapper, ['启动', 'Start']).trigger('click')
    await flushPromises()

    const newToasts = getToasts().value.slice(toastsBefore)
    const errorToast = newToasts.find((t) => t.type === 'error')
    expect(errorToast, '失败必须有 error toast').toBeTruthy()
    expect(errorToast!.message).toContain('port is already allocated')

    wrapper.unmount()
  })

  // Feature: dashboard-container-actions, Property: 操作进行中按钮必须禁用，
  // 且要等接口返回并完成状态刷新后才恢复 —— 防止用户以为点击无效而连点。
  it('启动进行中按钮禁用，完成并刷新状态后恢复', async () => {
    const wrapper = await mountedExitedApp()

    let resolveStart!: (value: unknown) => void
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === 'inspect_docker_host') return readyDockerReport()
      if (command === 'start_container') {
        return new Promise((resolve) => {
          resolveStart = resolve
        })
      }
      if (command === 'list_containers') {
        return [{ id: 'c1', name: 'ps-mysql80', image: 'mysql:8.0', status: 'Exited (0)', state: 'exited', ports: [] }]
      }
      return null
    })

    const btn = findCardButton(wrapper, ['启动', 'Start'])
    await btn.trigger('click')
    await flushPromises() // 只消化置位 busy 的微任务，不让 start_container 完成

    const busyBtn = findCardButton(wrapper, ['启动', 'Start'])
    expect(busyBtn.attributes('disabled'), '操作进行中应禁用按钮').toBeDefined()

    resolveStart(undefined)
    await flushPromises()
    await flushPromises()

    const restoredBtn = findCardButton(wrapper, ['启动', 'Start'])
    expect(restoredBtn.attributes('disabled'), '状态刷新后应恢复可用').toBeUndefined()

    wrapper.unmount()
  })
})
