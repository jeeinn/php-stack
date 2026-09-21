import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import WorkspaceMissingDialog from '../WorkspaceMissingDialog.vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}))

const INFO = {
  workspace_path: 'D:\\ps-missing',
  effective_path: 'E:\\study\\php-stack',
}

describe('WorkspaceMissingDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('打开时展示三选一按钮', () => {
    const wrapper = mount(WorkspaceMissingDialog, {
      props: { open: true, info: INFO },
    })
    expect(wrapper.find('[data-testid="workspace-missing-dialog"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="workspace-missing-recreate"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="workspace-missing-choose"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="workspace-missing-temp"]').exists()).toBe(true)
  })

  it('点重建成功后发出 resolved', async () => {
    vi.mocked(invoke).mockResolvedValue({ path_missing: false })
    const wrapper = mount(WorkspaceMissingDialog, {
      props: { open: true, info: INFO },
    })
    await wrapper.find('[data-testid="workspace-missing-recreate"]').trigger('click')
    await flushPromises()
    expect(invoke).toHaveBeenCalledWith('recreate_workspace_dir')
    expect(wrapper.emitted('resolved')).toHaveLength(1)
  })

  it('点临时回退发出 dismissTemp', async () => {
    const wrapper = mount(WorkspaceMissingDialog, {
      props: { open: true, info: INFO },
    })
    await wrapper.find('[data-testid="workspace-missing-temp"]').trigger('click')
    expect(wrapper.emitted('dismissTemp')).toHaveLength(1)
  })

  it('选新路径后调用 set_workspace_path 并 resolved', async () => {
    vi.mocked(open).mockResolvedValue('E:\\new-ws')
    vi.mocked(invoke).mockResolvedValue(undefined)
    const wrapper = mount(WorkspaceMissingDialog, {
      props: { open: true, info: INFO },
    })
    await wrapper.find('[data-testid="workspace-missing-choose"]').trigger('click')
    await flushPromises()
    expect(invoke).toHaveBeenCalledWith('set_workspace_path', { path: 'E:\\new-ws' })
    expect(wrapper.emitted('resolved')).toHaveLength(1)
  })
})
