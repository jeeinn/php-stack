import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import SoftwareSettings from '../SoftwareSettings.vue'

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === 'get_version_mappings') {
      return {
        php: [
          { id: 'php82', display_name: 'PHP 8.2', image_tag: 'php:8.2-fpm', service_dir: 'php82', default_port: 9000, show_port: false, eol: false, has_user_override: false },
          { id: 'php84', display_name: 'PHP 8.4', image_tag: 'php:8.4-fpm', service_dir: 'php84', default_port: 9000, show_port: false, eol: false, has_user_override: false },
        ],
        mysql: [
          { id: 'mysql80', display_name: 'MySQL 8.0', image_tag: 'mysql:8.0', service_dir: 'mysql80', default_port: 3306, show_port: true, eol: false, has_user_override: false },
        ],
        redis: [
          { id: 'redis72', display_name: 'Redis 7.2', image_tag: 'redis:7.2-alpine', service_dir: 'redis72', default_port: 6379, show_port: true, eol: false, has_user_override: false },
        ],
        nginx: [
          { id: 'nginx127', display_name: 'Nginx 1.27', image_tag: 'nginx:1.27-alpine', service_dir: 'nginx127', default_port: 80, show_port: true, eol: false, has_user_override: false },
        ],
      }
    }
    return null
  }),
}))

// Mock composables
vi.mock('../../composables/useToast', () => ({
  showToast: vi.fn(),
}))

vi.mock('../../composables/useConfirmDialog', () => ({
  showConfirm: vi.fn(async () => true),
}))

describe('SoftwareSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders the component', async () => {
    const wrapper = mount(SoftwareSettings)
    await flushPromises()
    expect(wrapper.exists()).toBe(true)
  })

  it('shows content after loading', async () => {
    const wrapper = mount(SoftwareSettings)
    await flushPromises()
    const content = wrapper.find('.flex-1')
    expect(content.exists()).toBe(true)
  })

  it('shows service tabs after loading', async () => {
    const wrapper = mount(SoftwareSettings)
    await flushPromises()
    const buttons = wrapper.findAll('button')
    const serviceTabs = buttons.filter(b => {
      const text = b.text()
      return ['PHP', 'MySQL', 'Redis', 'Nginx'].includes(text)
    })
    expect(serviceTabs.length).toBe(4)
  })

  it('shows version table after loading', async () => {
    const wrapper = mount(SoftwareSettings)
    await flushPromises()
    const table = wrapper.find('table')
    expect(table.exists()).toBe(true)
  })

  it('shows reset all button', async () => {
    const wrapper = mount(SoftwareSettings)
    await flushPromises()
    const buttons = wrapper.findAll('button')
    const resetButton = buttons.find(b => b.text().includes('Reset') || b.text().includes('重置'))
    expect(resetButton).toBeDefined()
  })

  it('shows edit button for each version', async () => {
    const wrapper = mount(SoftwareSettings)
    await flushPromises()
    const buttons = wrapper.findAll('button')
    const editButtons = buttons.filter(b => b.text().includes('Edit') || b.text().includes('编辑'))
    expect(editButtons.length).toBeGreaterThanOrEqual(1)
  })

  it('shows table headers', async () => {
    const wrapper = mount(SoftwareSettings)
    await flushPromises()
    const headers = wrapper.findAll('th')
    expect(headers.length).toBeGreaterThanOrEqual(5)
  })

  it('displays version names', async () => {
    const wrapper = mount(SoftwareSettings)
    await flushPromises()
    const codeElements = wrapper.findAll('code')
    expect(codeElements.length).toBeGreaterThanOrEqual(1)
  })
})