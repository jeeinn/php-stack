import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import WorkspaceInitDialog from '../WorkspaceInitDialog.vue'

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === 'get_workspace_info') {
      return { workspace_path: '/test/workspace' }
    }
    if (command === 'set_workspace_path') {
      return null
    }
    return null
  }),
}))

// Mock @tauri-apps/plugin-dialog
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(async () => '/new/workspace/path'),
}))

describe('WorkspaceInitDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders the component', async () => {
    const wrapper = mount(WorkspaceInitDialog)
    await flushPromises()
    expect(wrapper.exists()).toBe(true)
  })

  it('does not show dialog when workspace is configured', async () => {
    const wrapper = mount(WorkspaceInitDialog)
    await flushPromises()
    const dialog = wrapper.find('.fixed')
    expect(dialog.exists()).toBe(false)
  })

  it('shows dialog when workspace is not configured', async () => {
    vi.mocked(await import('@tauri-apps/api/core')).invoke.mockImplementation(async (command: string) => {
      if (command === 'get_workspace_info') {
        return null
      }
      return null
    })

    const wrapper = mount(WorkspaceInitDialog)
    await flushPromises()
    const dialog = wrapper.find('.fixed')
    expect(dialog.exists()).toBe(true)
  })

  it('shows dialog title', async () => {
    vi.mocked(await import('@tauri-apps/api/core')).invoke.mockImplementation(async (command: string) => {
      if (command === 'get_workspace_info') {
        return null
      }
      return null
    })

    const wrapper = mount(WorkspaceInitDialog)
    await flushPromises()
    const title = wrapper.find('h2')
    expect(title.exists()).toBe(true)
  })

  it('shows path input', async () => {
    vi.mocked(await import('@tauri-apps/api/core')).invoke.mockImplementation(async (command: string) => {
      if (command === 'get_workspace_info') {
        return null
      }
      return null
    })

    const wrapper = mount(WorkspaceInitDialog)
    await flushPromises()
    const input = wrapper.find('input[readonly]')
    expect(input.exists()).toBe(true)
  })

  it('shows path input field', async () => {
    vi.mocked(await import('@tauri-apps/api/core')).invoke.mockImplementation(async (command: string) => {
      if (command === 'get_workspace_info') {
        return null
      }
      return null
    })

    const wrapper = mount(WorkspaceInitDialog)
    await flushPromises()
    const input = wrapper.find('input')
    expect(input.exists()).toBe(true)
  })

  it('shows confirm button', async () => {
    vi.mocked(await import('@tauri-apps/api/core')).invoke.mockImplementation(async (command: string) => {
      if (command === 'get_workspace_info') {
        return null
      }
      return null
    })

    const wrapper = mount(WorkspaceInitDialog)
    await flushPromises()
    const buttons = wrapper.findAll('button')
    const confirmButton = buttons.find(b => b.text().includes('Start') || b.text().includes('开始') || b.text().includes('Confirm'))
    expect(confirmButton).toBeDefined()
  })
})