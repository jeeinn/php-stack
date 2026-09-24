import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import RestorePage from '../RestorePage.vue'
import { invoke } from '@tauri-apps/api/core'

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === 'preview_restore') {
      return {
        manifest: {
          timestamp: '2026-04-30T10:00:00Z',
          app_version: '0.4.0',
          os_info: 'Windows 11',
          services: [
            { name: 'ps-nginx', image: 'nginx', version: '1.27' },
            { name: 'ps-php', image: 'php', version: '8.4' },
          ],
          files: { '.env': {}, 'docker-compose.yml': {} },
          errors: [],
        },
        file_count: 2,
        port_conflicts: [],
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

// ─── U2 / R2：恢复结果明细展示 ──────────────────────────────────────
// 以前 execute_restore 只回 Ok/Err，前端弹一句 toast 就结束，
// 用户既看不全 restored_files 也来不及看 errors。现在改为始终返回结果，
// 这两条测试把「明细真的渲染出来了」锁住。

const PREVIEW_PAYLOAD = {
  manifest: {
    timestamp: '2026-04-30T10:00:00Z',
    app_version: '0.4.0',
    os_info: 'Windows 11',
    services: [{ name: 'ps-nginx', image: 'nginx', version: '1.27' }],
    files: { '.env': {}, 'docker-compose.yml': {} },
    errors: [],
  },
  file_count: 2,
  port_conflicts: [],
}

/** 走完 select → preview → verify，停在第 4 步，可执行恢复 */
async function prepareRestoreStep(wrapper: ReturnType<typeof mount>) {
  const vm = wrapper.vm as any
  await vm.selectFile()
  await vm.handlePreview()
  await vm.handleVerify()
  vm.goToStep('restore')
  await flushPromises()
}

function mockInvoke(executeRestoreResult: unknown) {
  vi.mocked(invoke).mockImplementation(async (command: string) => {
    if (command === 'preview_restore') return PREVIEW_PAYLOAD
    if (command === 'verify_backup') return true
    if (command === 'execute_restore') return executeRestoreResult
    return null
  })
}

describe('RestorePage — 恢复结果明细', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('成功时展示已恢复文件列表与回滚包路径', async () => {
    mockInvoke({
      success: true,
      restored_files: ['.env', 'docker-compose.yml', 'services/php82/php.ini'],
      errors: [],
      rollback_path: '/ws/.restore_rollback_20260921_120000.zip',
    })

    const wrapper = mount(RestorePage)
    await prepareRestoreStep(wrapper)
    await (wrapper.vm as any).handleRestore()
    await flushPromises()

    const files = wrapper.find('[data-testid="restored-files"]')
    expect(files.exists()).toBe(true)
    expect(files.text()).toContain('.env')
    expect(files.text()).toContain('services/php82/php.ini')

    // R2：回滚包路径必须可见，否则用户不知道去哪撤销
    const rollback = wrapper.find('[data-testid="rollback-path"]')
    expect(rollback.exists()).toBe(true)
    expect(rollback.text()).toContain('.restore_rollback_')

    // 成功时不应出现错误区块
    expect(wrapper.find('[data-testid="restore-errors"]').exists()).toBe(false)
  })

  it('部分失败时逐条展示错误明细，不折叠成一句话 toast', async () => {
    mockInvoke({
      success: false,
      restored_files: ['.env'],
      errors: ['恢复 services/ 失败: 权限不足', '恢复 database/ 失败: 条目损坏'],
      rollback_path: '/ws/.restore_rollback_20260921_120001.zip',
    })

    const wrapper = mount(RestorePage)
    await prepareRestoreStep(wrapper)
    await (wrapper.vm as any).handleRestore()
    await flushPromises()

    const errors = wrapper.find('[data-testid="restore-errors"]')
    expect(errors.exists()).toBe(true)
    expect(errors.text()).toContain('恢复 services/ 失败: 权限不足')
    expect(errors.text()).toContain('恢复 database/ 失败: 条目损坏')

    // 部分失败同样要给出回滚包，这是用户唯一的退路
    expect(wrapper.find('[data-testid="rollback-path"]').text()).toContain(
      '.restore_rollback_',
    )
    expect(wrapper.find('[data-testid="rollback-action"]').exists()).toBe(true)
    // 有已恢复文件 → 不算致命失败
    expect(wrapper.find('[data-testid="restore-fatal"]').exists()).toBe(false)
  })

  it('致命失败时展示错误面板与回滚快捷入口，而非仅 toast', async () => {
    mockInvoke({
      success: false,
      restored_files: [],
      errors: ['备份包包含非法路径条目，已拒绝恢复: ../../escaped.txt'],
      rollback_path: '/ws/.restore_rollback_20260921_120002.zip',
    })

    const wrapper = mount(RestorePage)
    await prepareRestoreStep(wrapper)
    await (wrapper.vm as any).handleRestore()
    await flushPromises()

    expect(wrapper.find('[data-testid="restore-fatal"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="restore-errors"]').text()).toContain(
      '非法路径条目',
    )
    expect(wrapper.find('[data-testid="rollback-action"]').exists()).toBe(true)
  })

  it('点击回滚快捷入口后载入回滚包并进入预览步骤', async () => {
    const rollbackPath = '/ws/.restore_rollback_20260921_120003.zip'
    mockInvoke({
      success: false,
      restored_files: [],
      errors: ['打开备份文件失败'],
      rollback_path: rollbackPath,
    })

    const wrapper = mount(RestorePage)
    await prepareRestoreStep(wrapper)
    await (wrapper.vm as any).handleRestore()
    await flushPromises()

    await (wrapper.vm as any).useRollbackBundle()
    await flushPromises()

    const vm = wrapper.vm as any
    expect(vm.zipPath).toBe(rollbackPath)
    expect(vm.currentStep).toBe('preview')
    expect(vm.restoreResult).toBeNull()
    // 载入后自动触发预览
    expect(vm.preview).not.toBeNull()
  })
})