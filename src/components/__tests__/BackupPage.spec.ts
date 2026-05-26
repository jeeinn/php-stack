import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount } from '@vue/test-utils'
import BackupPage from '../BackupPage.vue'

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === 'convert_to_relative_path') {
      return './relative/path'
    }
    return null
  }),
}))

// Mock @tauri-apps/plugin-dialog
vi.mock('@tauri-apps/plugin-dialog', () => ({
  save: vi.fn(async () => '/test/backup.zip'),
  open: vi.fn(async () => '/test/selected/path'),
}))

// Mock @tauri-apps/api/event
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => vi.fn()),
}))

// Mock composables
vi.mock('../../composables/useToast', () => ({
  showToast: vi.fn(),
  addLog: vi.fn(),
}))

describe('BackupPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders the component', () => {
    const wrapper = mount(BackupPage)
    expect(wrapper.exists()).toBe(true)
  })

  it('displays the title', () => {
    const wrapper = mount(BackupPage)
    const title = wrapper.find('h1')
    expect(title.exists()).toBe(true)
  })

  it('displays backup options section', () => {
    const wrapper = mount(BackupPage)
    const sections = wrapper.findAll('section')
    expect(sections.length).toBeGreaterThanOrEqual(1)
  })

  it('has core config checkbox checked and disabled', () => {
    const wrapper = mount(BackupPage)
    const checkboxes = wrapper.findAll('input[type="checkbox"]')
    const coreConfigCheckbox = checkboxes[0]
    expect(coreConfigCheckbox.attributes('checked')).toBeDefined()
    expect(coreConfigCheckbox.attributes('disabled')).toBeDefined()
  })

  it('has include projects checkbox', () => {
    const wrapper = mount(BackupPage)
    const checkboxes = wrapper.findAll('input[type="checkbox"]')
    expect(checkboxes.length).toBeGreaterThanOrEqual(2)
  })

  it('has include logs checkbox', () => {
    const wrapper = mount(BackupPage)
    const checkboxes = wrapper.findAll('input[type="checkbox"]')
    expect(checkboxes.length).toBeGreaterThanOrEqual(3)
  })

  it('has create backup button', () => {
    const wrapper = mount(BackupPage)
    const button = wrapper.find('button')
    expect(button.exists()).toBe(true)
  })

  it('shows project patterns textarea when include projects is checked', async () => {
    const wrapper = mount(BackupPage)
    const checkboxes = wrapper.findAll('input[type="checkbox"]')
    const includeProjectsCheckbox = checkboxes[1]

    await includeProjectsCheckbox.setValue(true)
    await wrapper.vm.$nextTick()

    const textarea = wrapper.find('textarea')
    expect(textarea.exists()).toBe(true)
  })

  it('does not show project patterns textarea when include projects is unchecked', () => {
    const wrapper = mount(BackupPage)
    const textarea = wrapper.find('textarea')
    expect(textarea.exists()).toBe(false)
  })
})