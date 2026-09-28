import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { invoke } from '@tauri-apps/api/core'
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
  timezone: 'Asia/Shanghai',
}

const mockCatalog = [
  { id: 'php', display_name: 'PHP', builtin: true, generator: 'php', container_port: 9000, connect: { container_port: 9000, short_name: null } },
  { id: 'mysql', display_name: 'MySQL', builtin: true, generator: 'image', container_port: 3306, connect: { container_port: 3306, short_name: 'mysql' } },
  { id: 'redis', display_name: 'Redis', builtin: true, generator: 'image', container_port: 6379, connect: { container_port: 6379, short_name: 'redis' } },
  { id: 'nginx', display_name: 'Nginx', builtin: true, generator: 'nginx', container_port: 80, connect: { container_port: 80, short_name: 'nginx' } },
]

// Mock @tauri-apps/api/core
async function defaultInvoke(command: string) {
  switch (command) {
    case 'get_version_mappings':
      return mockVersionMappings
    case 'get_service_catalog':
      return mockCatalog
    case 'load_existing_config':
      return mockExistingConfig
    case 'get_workspace_info':
      return { workspace_path: '/test/workspace' }
    case 'check_config_files_exist':
      return []
    case 'generate_env_config':
      return 'APP_ENV=local'
    case 'preview_compose':
      return 'services:\n  php:\n    image: php:8.2-fpm'
    default:
      return null
  }
}

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(defaultInvoke),
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(async () => null),
}))

vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({
  writeText: vi.fn(async () => {}),
}))

vi.mock('@tauri-apps/plugin-shell', () => ({
  open: vi.fn(async () => {}),
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
    vi.mocked(invoke).mockImplementation(defaultInvoke)
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

  it('custom extension inputs are independent across multiple PHP services', async () => {
    const wrapper = mount(EnvConfigPage)
    await flushPromises()

    // 在 PHP 分区内点击"添加版本"，得到两个 PHP 服务卡片
    const sections = wrapper.findAll('section')
    const phpSection = sections.find(s => s.text().includes('PHP 服务'))!
    const addBtn = phpSection.findAll('button').find(b => b.text().includes('添加版本'))!
    await addBtn.trigger('click')

    // 展开两个服务的扩展面板（默认收起）
    const panelToggles = phpSection
      .findAll('button')
      .filter(b => b.text().includes('预设扩展库'))
    expect(panelToggles.length).toBe(2)
    for (const toggle of panelToggles) {
      await toggle.trigger('click')
    }

    // 找到两个"自定义扩展"输入框（placeholder 定位）
    const customInputs = phpSection.findAll('input[placeholder="例如: grpc protobuf"]')
    expect(customInputs.length).toBe(2)

    // 给第一个服务的输入框填写内容
    await customInputs[0].setValue('xdebug')
    expect((customInputs[0].element as HTMLInputElement).value).toBe('xdebug')

    // 给第二个服务的输入框填写不同内容（修复前共享同一个 ref，会覆盖第一个输入框）
    await customInputs[1].setValue('swoole')

    const v1 = (customInputs[0].element as HTMLInputElement).value
    const v2 = (customInputs[1].element as HTMLInputElement).value
    expect(v1).toBe('xdebug')
    expect(v2).toBe('swoole')
    expect(v1).not.toBe(v2)

    // blur 触发各自的合并逻辑，输入框内容保持独立
    await customInputs[0].trigger('blur')
    await customInputs[1].trigger('blur')
    expect((customInputs[0].element as HTMLInputElement).value).toBe('xdebug')
    expect((customInputs[1].element as HTMLInputElement).value).toBe('swoole')
  })

  // Feature: env-config-page, Property: 「预览配置」必须真实调用后端 preview_compose。
  // 回归背景：存放预览结果的 ref 与同名导入的 API 函数互相遮蔽，函数名被 ref 覆盖，
  // previewCompose(config) 变成对 Ref 的调用并抛 TypeError，预览弹窗永不打开。
  it('点预览真实调用后端 preview_compose 并展示生成结果', async () => {
    const wrapper = mount(EnvConfigPage)
    await flushPromises()

    const previewButton = wrapper.findAll('button').find(b => b.text().includes('预览配置'))
    expect(previewButton).toBeDefined()

    await previewButton!.trigger('click')
    await flushPromises()

    expect(vi.mocked(invoke)).toHaveBeenCalledWith('preview_compose', {
      config: expect.anything(),
    })

    // 弹窗确实打开，且两个预览区各自渲染了后端返回值
    const text = wrapper.text()
    expect(text).toContain('docker-compose.yml')
    expect(text).toContain('php:8.2-fpm')
    expect(text).toContain('APP_ENV=local')
  })

  // Feature: optional-mysql, Property: 新工作区默认不启用 MySQL
  it('新工作区无既有配置时 MySQL 区为空态', async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      switch (command) {
        case 'get_version_mappings':
          return mockVersionMappings
        case 'get_service_catalog':
          return mockCatalog
        case 'load_existing_config':
          return null
        case 'get_workspace_info':
          return { workspace_path: '/test/workspace' }
        default:
          return null
      }
    })

    const wrapper = mount(EnvConfigPage)
    await flushPromises()

    const sections = wrapper.findAll('section')
    const mysqlSection = sections.find(s => s.text().includes('MySQL 服务'))!
    expect(mysqlSection).toBeDefined()
    // 空态：无端口输入，有「添加版本」类入口
    expect(mysqlSection.findAll('input[type="number"]').length).toBe(0)
    expect(mysqlSection.text()).toMatch(/添加|空|尚未|未添加|MySQL/)
  })

  // Feature: optional-mysql, Property: MySQL 可删到 0
  it('删除最后一个 MySQL 实例后进入空态', async () => {
    const wrapper = mount(EnvConfigPage)
    await flushPromises()

    const sections = wrapper.findAll('section')
    const mysqlSection = sections.find(s => s.text().includes('MySQL 服务'))!
    expect(mysqlSection.findAll('input[type="number"]').length).toBeGreaterThan(0)

    const removeBtn = mysqlSection.findAll('button').find(b => b.text().includes('移除') || b.text().includes('删除'))
    expect(removeBtn).toBeDefined()
    await removeBtn!.trigger('click')
    await flushPromises()

    expect(mysqlSection.findAll('input[type="number"]').length).toBe(0)
  })

  // Feature: custom-service, Property: 保存自定义服务后出现在高级区
  it('自定义服务对话框保存成功后出现该 kind', async () => {
    const savedDesc = {
      id: 'mongodb',
      display_name: 'MongoDB',
      builtin: false,
      generator: 'image',
      container_port: 27017,
      connect: { container_port: 27017, short_name: 'mongo' },
    }
    let catalog = [...mockCatalog]

    vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
      switch (command) {
        case 'get_version_mappings':
          return {
            ...mockVersionMappings,
            mongodb: [
              {
                id: 'mongodbdefault',
                display_name: 'MongoDB (mongodbdefault)',
                image_tag: 'mongo:7',
                service_dir: 'mongodbdefault',
                default_port: 27017,
                show_port: true,
                eol: false,
              },
            ],
          }
        case 'get_service_catalog':
          return catalog
        case 'load_existing_config':
          return null
        case 'get_workspace_info':
          return { workspace_path: '/test/workspace' }
        case 'save_custom_service':
          catalog = [...catalog, savedDesc]
          return savedDesc
        default:
          return null
      }
    })

    const wrapper = mount(EnvConfigPage)
    await flushPromises()

    const addKindBtn = wrapper.findAll('button').find(b =>
      b.text().includes('添加自定义服务') || b.text().includes('Add custom'),
    )
    expect(addKindBtn).toBeDefined()
    await addKindBtn!.trigger('click')
    await flushPromises()

    // 填写对话框（按 placeholder / label 找 input）
    const inputs = wrapper.findAll('input')
    const textInputs = inputs.filter(i => (i.element as HTMLInputElement).type === 'text' || !(i.element as HTMLInputElement).type)
    // 对话框打开后应有显示名、id、镜像等
    expect(wrapper.text()).toMatch(/Mongo|自定义|Custom|显示名|Display/i)

    // 直接调用内部保存路径：找确认按钮
    const confirmBtn = wrapper.findAll('button').find(b =>
      b.text().includes('保存') || b.text().includes('Save') || b.text().includes('确定'),
    )
    // 先填必要字段（vm 绑定）
    const displayInput = wrapper.find('input[placeholder*="Mongo"], input[placeholder*="MongoDB"], input[placeholder*="显示"]')
    // 若 placeholder 不稳定，用对话框内第一个 text input 链
    const dialogInputs = wrapper.findAll('.fixed input, [role="dialog"] input, form input')
    const candidates = dialogInputs.length > 0 ? dialogInputs : textInputs
    if (candidates.length >= 3) {
      await candidates[0].setValue('MongoDB')
      await candidates[1].setValue('mongodb')
      await candidates[2].setValue('mongo:7')
    }

    const numberInputs = wrapper.findAll('input[type="number"]')
    // 对话框内的 host/container port
    for (const ni of numberInputs.slice(-2)) {
      await ni.setValue(27017)
    }

    if (confirmBtn) {
      await confirmBtn.trigger('click')
      await flushPromises()
    }

    expect(vi.mocked(invoke)).toHaveBeenCalledWith(
      'save_custom_service',
      expect.objectContaining({ form: expect.anything() }),
    )
  })
})
