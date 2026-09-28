import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { invoke } from '@tauri-apps/api/core'
import EnvConfigPage from '../EnvConfigPage.vue'

/**
 * 覆盖 v0.5 NEW-5 的前端部分：
 * 1. 编辑已有自定义 kind 时 id / version_id 被锁定（避免改 id 产生孤儿 version override）
 * 2. 编辑提交走 update_custom_service（此前该命令零调用方、零测试覆盖）
 * 3. 新建态不锁定，作为上述契约的反向对照
 */

const mockPhpVersions = [
  { id: 'php82', display_name: 'PHP 8.2', image_tag: 'php:8.2-fpm', service_dir: 'php82', default_port: 9000, show_port: false, eol: false },
]

const mockMongoVersions = [
  { id: 'mongodbdefault', display_name: 'MongoDB (mongodbdefault)', image_tag: 'mongo:7', service_dir: 'mongodbdefault', default_port: 27017, show_port: true, eol: false },
]

const mockVersionMappings = {
  php: mockPhpVersions,
  mongodb: mockMongoVersions,
}

const mockCatalog = [
  { id: 'php', display_name: 'PHP', builtin: true, generator: 'php', container_port: 9000, connect: { container_port: 9000, short_name: 'php' } },
  {
    id: 'mongodb',
    display_name: 'MongoDB',
    builtin: false,
    generator: 'image',
    container_port: 27017,
    short_alias: 'mongo',
    volumes: { data: { container_path: '/data/db' } },
    connect: { container_port: 27017, short_name: 'mongo' },
  },
]

const mockExistingConfig = {
  services: [{ service_type: 'PHP', version: 'php82', host_port: 9000 }],
  timezone: 'Asia/Shanghai',
}

let calls: string[] = []

async function defaultInvoke(command: string, args?: Record<string, any>) {
  calls.push(command)
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
    case 'save_custom_service':
    case 'update_custom_service':
      return mockCatalog.find(s => s.id === args?.form?.id) ?? mockCatalog[1]
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

vi.mock('../../composables/useToast', () => ({
  showToast: vi.fn(),
}))

vi.mock('../../composables/useConfirmDialog', () => ({
  showConfirm: vi.fn(),
}))

function customSection(wrapper: ReturnType<typeof mount>) {
  const section = wrapper
    .findAll('section')
    .find(s => s.text().includes('自定义服务'))
  expect(section).toBeDefined()
  return section!
}

function dialogTitles(wrapper: ReturnType<typeof mount>) {
  return wrapper.findAll('h2').map(h => h.text()).join(' | ')
}

function buttonByText(wrapper: ReturnType<typeof mount>, text: string) {
  const btn = wrapper.findAll('button').find(b => b.text() === text)
  expect(btn).toBeDefined()
  return btn!
}

const ID_LABEL = '服务 ID'
const VERSION_ID_LABEL = '版本 ID'

/** 按 label 文本定位其后的 input（versionId 的 placeholder 含 {id} 插值，不宜作锚点） */
function inputAfterLabel(wrapper: ReturnType<typeof mount>, labelText: string): HTMLInputElement {
  const labels = Array.from(wrapper.element.querySelectorAll('label')) as HTMLLabelElement[]
  const label = labels.find(l => l.textContent?.includes(labelText))
  expect(label).toBeDefined()
  const input = label!.parentElement?.querySelector('input') as HTMLInputElement | null
  expect(input).not.toBeNull()
  return input!
}

describe('EnvConfigPage custom service editing', () => {
  beforeEach(() => {
    calls = []
    vi.clearAllMocks()
    vi.mocked(invoke).mockImplementation(defaultInvoke as any)
  })

  it('editing a custom kind locks id and version_id inputs', async () => {
    const wrapper = mount(EnvConfigPage)
    await flushPromises()

    const editBtn = customSection(wrapper)
      .findAll('button')
      .find(b => b.text() === '编辑')
    expect(editBtn).toBeDefined()
    await editBtn!.trigger('click')
    await flushPromises()

    expect(dialogTitles(wrapper)).toContain('编辑自定义服务')

    const idInput = inputAfterLabel(wrapper, ID_LABEL)
    const versionInput = inputAfterLabel(wrapper, VERSION_ID_LABEL)

    expect(idInput.disabled).toBe(true)
    expect(versionInput.disabled).toBe(true)

    // 编辑态回填该 kind 当前的 id / version_id，而不是空值或默认拼接值
    expect(versionInput.value).toBe('mongodbdefault')
    expect(idInput.value).toBe('mongodb')
  })

  it('editing submits update_custom_service instead of save_custom_service', async () => {
    const wrapper = mount(EnvConfigPage)
    await flushPromises()

    const editBtn = customSection(wrapper)
      .findAll('button')
      .find(b => b.text() === '编辑')
    await editBtn!.trigger('click')
    await flushPromises()

    await buttonByText(wrapper, '保存').trigger('click')
    await flushPromises()
    await flushPromises()

    expect(calls).toContain('update_custom_service')
    expect(calls).not.toContain('save_custom_service')
    expect(calls.filter(c => c === 'update_custom_service').length).toBe(1)

    // 保存成功后对话框关闭，编辑态复位
    expect(dialogTitles(wrapper)).not.toContain('编辑自定义服务')
  })

  it('creating a new custom kind keeps id editable (no lock)', async () => {
    const wrapper = mount(EnvConfigPage)
    await flushPromises()

    const addBtn = customSection(wrapper)
      .findAll('button')
      .find(b => b.text() === '添加自定义服务')
    expect(addBtn).toBeDefined()
    await addBtn!.trigger('click')
    await flushPromises()

    expect(dialogTitles(wrapper)).toContain('添加自定义服务')

    const idInput = inputAfterLabel(wrapper, ID_LABEL)
    const versionInput = inputAfterLabel(wrapper, VERSION_ID_LABEL)
    expect(idInput.disabled).toBe(false)
    expect(versionInput.disabled).toBe(false)
    expect(idInput.value).toBe('')
  })
})
