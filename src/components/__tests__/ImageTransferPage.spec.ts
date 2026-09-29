import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import ImageTransferPage from '../ImageTransferPage.vue'
import zhCN from '../../i18n/locales/zh-CN.json'
import { showToast } from '../../composables/useToast'

const listMock = vi.fn()
const exportMock = vi.fn()

vi.mock('../../api', async () => {
  const actual = await vi.importActual('../../api')
  return {
    ...actual,
    listWorkspaceImages: (...args: unknown[]) => listMock(...args),
    exportWorkspaceImages: (...args: unknown[]) => exportMock(...args),
    importWorkspaceImages: vi.fn(),
    normalizeError: (e: unknown) => String(e),
  }
})

vi.mock('@tauri-apps/plugin-dialog', () => ({
  save: vi.fn(async () => '/test/images.tar'),
  open: vi.fn(async () => '/test/images.tar'),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => vi.fn()),
}))

vi.mock('../../composables/useToast', () => ({
  showToast: vi.fn(),
  addLog: vi.fn(),
}))

const defaultList = [
  {
    ref_name: 'php:8.2-fpm',
    role: 'base',
    service_dir: 'php82',
    service_kind: 'php',
    present: true,
    size: '691MB',
  },
  {
    ref_name: 'php-stack/php82:abc',
    role: 'built',
    service_dir: 'php82',
    service_kind: 'php',
    present: false,
    note: 'built_missing_will_build_on_target',
  },
]

describe('ImageTransferPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    listMock.mockResolvedValue(defaultList)
  })

  it('renders title and loads workspace images', async () => {
    const wrapper = mount(ImageTransferPage)
    await flushPromises()
    expect(wrapper.find('h1').exists()).toBe(true)
    expect(listMock).toHaveBeenCalled()
    expect(wrapper.text()).toContain('php:8.2-fpm')
  })

  it('has export and import action buttons', async () => {
    const wrapper = mount(ImageTransferPage)
    await flushPromises()
    const text = wrapper.text()
    expect(text).toContain(zhCN.images.action.export)
    expect(text).toContain(zhCN.images.action.import)
  })

  it('passes apply action into hint i18n', async () => {
    const wrapper = mount(ImageTransferPage)
    await flushPromises()
    const hint = wrapper.find('header p.text-amber-700')
    expect(hint.exists()).toBe(true)
    // 守住 P0：hint 必须把 envConfig.apply 插进 {action}，不能只渲染空壳
    expect(hint.text()).toContain(zhCN.envConfig.apply)
    expect(hint.text()).not.toContain('{action}')
  })

  it('shows exportPartial toast when some refs are skipped', async () => {
    exportMock.mockResolvedValueOnce({
      exported: ['php:8.2-fpm'],
      skipped: ['php-stack/php82:abc'],
      tar_path: '/test/images.tar',
      manifest_path: '/test/images.manifest.json',
    })

    const wrapper = mount(ImageTransferPage)
    await flushPromises()

    const exportBtn = wrapper
      .findAll('button')
      .find((b) => b.text().includes(zhCN.images.action.export))
    expect(exportBtn).toBeTruthy()
    await exportBtn!.trigger('click')
    await flushPromises()

    expect(exportMock).toHaveBeenCalled()
    // 中文：已导出 {exported} 个镜像到 {path}（跳过缺失 {skipped} 个）
    expect(vi.mocked(showToast)).toHaveBeenCalledWith(
      expect.stringMatching(/已导出 1 个镜像.*跳过缺失 1/),
      'info',
    )
  })
})
