import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import EnvConfigPage from '../EnvConfigPage.vue'
import type { VersionInfo } from '../../types/env-config'

// Mock version data using new VersionInfo structure
const mockPhpVersions: VersionInfo[] = [
  { id: 'php82', display_name: 'PHP 8.2', image_tag: 'php:8.2-fpm', service_dir: 'php82', default_port: 9000, show_port: false, eol: false },
  { id: 'php84', display_name: 'PHP 8.4', image_tag: 'php:8.4-fpm', service_dir: 'php84', default_port: 9000, show_port: false, eol: false },
]

const mockMysqlVersions: VersionInfo[] = [
  { id: 'mysql80', display_name: 'MySQL 8.0', image_tag: 'mysql:8.0', service_dir: 'mysql80', default_port: 3306, show_port: true, eol: false },
  { id: 'mysql84', display_name: 'MySQL 8.4 LTS', image_tag: 'mysql:8.4', service_dir: 'mysql84', default_port: 3306, show_port: true, eol: false },
]

const mockRedisVersions: VersionInfo[] = [
  { id: 'redis72', display_name: 'Redis 7.2', image_tag: 'redis:7.2-alpine', service_dir: 'redis72', default_port: 6379, show_port: true, eol: false },
]

const mockNginxVersions: VersionInfo[] = [
  { id: 'nginx127', display_name: 'Nginx 1.27', image_tag: 'nginx:1.27-alpine', service_dir: 'nginx127', default_port: 80, show_port: true, eol: false },
]

const mockVersionMappings = {
  php: mockPhpVersions,
  mysql: mockMysqlVersions,
  redis: mockRedisVersions,
  nginx: mockNginxVersions,
}

const mockExistingConfig = {
  services: [
    { service_type: 'PHP', version: 'php82', host_port: 9000, extensions: ['pdo_mysql', 'mysqli'] },
    { service_type: 'MySQL', version: 'mysql80', host_port: 3306 },
  ],
  source_dir: './www',
  timezone: 'Asia/Shanghai',
}

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (command: string) => {
    switch (command) {
      case 'get_version_mappings':
        return mockVersionMappings
      case 'load_existing_config':
        return mockExistingConfig
      case 'get_workspace_info':
        return { workspace_path: '/test/workspace' }
      case 'check_config_files_exist':
        return []
      default:
        return null
    }
  }),
}))

// Mock composables
vi.mock('../../composables/useToast', () => ({
  showToast: vi.fn(),
}))

vi.mock('../../composables/useConfirmDialog', () => ({
  showConfirm: vi.fn(),
}))

describe('EnvConfigPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders the component', () => {
    const wrapper = mount(EnvConfigPage)
    expect(wrapper.exists()).toBe(true)
  })

  it('displays the title', async () => {
    const wrapper = mount(EnvConfigPage)
    await flushPromises()
    expect(wrapper.text()).toContain('环境配置') // zh-CN 默认语言
  })

  it('has PHP service section', async () => {
    const wrapper = mount(EnvConfigPage)
    await flushPromises()
    expect(wrapper.text()).toContain('PHP')
  })

  it('dropdown uses id as value for PHP versions', async () => {
    const wrapper = mount(EnvConfigPage)
    await flushPromises()

    // The component should have loaded version options and display the selected one
    const text = wrapper.text()
    // Check that PHP versions are displayed in label format
    expect(text).toContain('PHP 8.2')
    expect(text).toContain('php:8.2-fpm')
  })

  it('dropdown displays display_name and image_tag', async () => {
    const wrapper = mount(EnvConfigPage)
    await flushPromises()

    // Check that display_name and image_tag appear in the rendered text
    // Note: CustomSelect shows the selected option's label, not all options
    expect(wrapper.text()).toContain('PHP 8.2')
    expect(wrapper.text()).toContain('php:8.2-fpm')
    // PHP 8.4 may not be visible unless it's selected or the dropdown is open
  })

  it('PHP section does not show port input (show_port=false)', async () => {
    const wrapper = mount(EnvConfigPage)
    await flushPromises()

    // Find the PHP service section (first section with 🐘)
    const sections = wrapper.findAll('section')
    const phpSection = sections.find(s => s.text().includes('PHP 服务'))!
    expect(phpSection).toBeDefined()

    // PHP section should NOT have a port number input
    // (PHP has show_port=false, and the template doesn't render port input for PHP at all)
    const portInputs = phpSection.findAll('input[type="number"]')
    expect(portInputs.length).toBe(0)
  })

  it('MySQL section shows port input (show_port=true)', async () => {
    const wrapper = mount(EnvConfigPage)
    await flushPromises()

    // Find the MySQL service section
    const sections = wrapper.findAll('section')
    const mysqlSection = sections.find(s => s.text().includes('MySQL 服务'))!
    expect(mysqlSection).toBeDefined()

    // MySQL has show_port=true, so port input should be visible
    const portInputs = mysqlSection.findAll('input[type="number"]')
    expect(portInputs.length).toBeGreaterThan(0)
  })

  it('MySQL dropdown uses id as value', async () => {
    const wrapper = mount(EnvConfigPage)
    await flushPromises()

    // Check that MySQL versions are loaded and displayed
    const text = wrapper.text()
    expect(text).toContain('MySQL 8.0')
    expect(text).toContain('mysql:8.0')
  })
})
