import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { invoke } from '@tauri-apps/api/core'
import App from '../App.vue'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (command: string) => {
    switch (command) {
      case 'check_docker':
        return undefined
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
  getVersion: vi.fn(async () => '0.3.1'),
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

  // Feature: dashboard-docker-check, Property: 挂载后必须真实调用后端 check_docker，
  // 并在通过后继续拉取容器列表。
  // 回归背景：局部包装函数与同名导入的 API 函数互相遮蔽，函数体内的 await 调用的是
  // 它自己，形成同步无限递归并抛 RangeError、被 catch 当作「Docker 不可用」吞掉；
  // check_docker 与 list_containers 均不会执行，仪表盘恒显示不可用。
  it('挂载后真实调用 check_docker 并接着拉取容器列表', async () => {
    const wrapper = mount(App)
    await flushPromises()
    await flushPromises()

    const commands = vi.mocked(invoke).mock.calls.map(c => c[0])
    expect(commands).toContain('check_docker')
    expect(commands).toContain('list_containers')

    wrapper.unmount()
  })
})
