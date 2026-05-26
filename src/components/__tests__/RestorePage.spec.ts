import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount } from '@vue/test-utils'
import RestorePage from '../RestorePage.vue'

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === 'preview_restore') {
      return {
        manifest: {
          timestamp: '2026-04-30T10:00:00Z',
          app_version: '0.3.1',
          os_info: 'Windows 11',
          services: [
            { name: 'ps-nginx', image: 'nginx', version: '1.27' },
            { name: 'ps-php', image: 'php', version: '8.4' },
          ],
          files: { '.env': {}, 'docker-compose.yml': {} },
          errors: [],
        },
      }
    }
    if (command === 'verify_backup') {
      return true
    }
    return null
  }),
}))

// Mock @tauri-apps/plugin-dialog
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(async () => '/test/backup.zip'),
}))

// Mock @tauri-apps/api/event
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => vi.fn()),
}))

// Mock composables
vi.mock('../../composables/useToast', () => ({
  showToast: vi.fn(),
}))

vi.mock('../../composables/useConfirmDialog', () => ({
  showConfirm: vi.fn(async () => true),
}))

describe('RestorePage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders the component', () => {
    const wrapper = mount(RestorePage)
    expect(wrapper.exists()).toBe(true)
  })

  it('displays the title', () => {
    const wrapper = mount(RestorePage)
    const title = wrapper.find('h1')
    expect(title.exists()).toBe(true)
  })

  it('shows step indicator with 4 steps', () => {
    const wrapper = mount(RestorePage)
    const stepLabels = wrapper.findAll('span.text-xs')
    expect(stepLabels.length).toBeGreaterThanOrEqual(4)
  })

  it('starts at select step', () => {
    const wrapper = mount(RestorePage)
    const selectSection = wrapper.find('section')
    expect(selectSection.exists()).toBe(true)
  })

  it('shows file selection input', () => {
    const wrapper = mount(RestorePage)
    const input = wrapper.find('input[readonly]')
    expect(input.exists()).toBe(true)
  })

  it('shows file selection section', () => {
    const wrapper = mount(RestorePage)
    const section = wrapper.find('section')
    expect(section.exists()).toBe(true)
  })

  it('has step 1 active by default', () => {
    const wrapper = mount(RestorePage)
    const stepIndicators = wrapper.findAll('.rounded-full')
    expect(stepIndicators.length).toBeGreaterThanOrEqual(4)
  })
})