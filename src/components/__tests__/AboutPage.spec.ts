import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import AboutPage from '../AboutPage.vue'
import { LOG_LEVEL_STORAGE_KEY } from '../../composables/useToast'

const getSupportInfo = vi.fn()
const exportLogsTo = vi.fn()
const writeText = vi.fn()
const save = vi.fn()
const open = vi.fn()
const check = vi.fn()
const relaunch = vi.fn()

vi.mock('../../api', () => ({
  getSupportInfo: (...args: unknown[]) => getSupportInfo(...args),
  exportLogsTo: (...args: unknown[]) => exportLogsTo(...args),
}))

vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({
  writeText: (...args: unknown[]) => writeText(...args),
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({
  save: (...args: unknown[]) => save(...args),
}))

vi.mock('@tauri-apps/plugin-shell', () => ({
  open: (...args: unknown[]) => open(...args),
}))

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: (...args: unknown[]) => check(...args),
}))

vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: (...args: unknown[]) => relaunch(...args),
}))

vi.mock('../../composables/useToast', async () => {
  const actual = await vi.importActual<typeof import('../../composables/useToast')>(
    '../../composables/useToast',
  )
  return {
    ...actual,
    showToast: vi.fn(),
  }
})

describe('AboutPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    localStorage.removeItem(LOG_LEVEL_STORAGE_KEY)
    getSupportInfo.mockResolvedValue({
      app_version: '0.4.0',
      os: 'Windows',
      os_version: '10.0.26120',
      arch: 'x86_64',
    })
  })

  it('renders support info and copies it', async () => {
    const wrapper = mount(AboutPage)
    await flushPromises()
    expect(wrapper.get('[data-testid="about-version"]').text()).toBe('0.4.0')
    expect(wrapper.get('[data-testid="about-os"]').text()).toContain('Windows')
    expect(wrapper.get('[data-testid="about-arch"]').text()).toBe('x86_64')

    await wrapper.get('[data-testid="about-copy-support"]').trigger('click')
    expect(writeText).toHaveBeenCalledWith(
      'Version: 0.4.0\nOS: Windows 10.0.26120\nArch: x86_64',
    )
  })

  it('changes log level', async () => {
    const wrapper = mount(AboutPage)
    await flushPromises()
    const buttons = wrapper.get('[data-testid="about-log-level"]').findAll('button')
    expect(buttons.length).toBe(3)
    await buttons[2].trigger('click')
    expect(localStorage.getItem(LOG_LEVEL_STORAGE_KEY)).toBe('error')
  })

  it('opens the project repository', async () => {
    const wrapper = mount(AboutPage)
    await flushPromises()
    await wrapper.get('[data-testid="about-open-repo"]').trigger('click')
    expect(open).toHaveBeenCalledWith('https://github.com/jeeinn/php-stack')
  })

  it('shows up to date when check returns null', async () => {
    check.mockResolvedValueOnce(null)
    const wrapper = mount(AboutPage)
    await flushPromises()
    await wrapper.get('[data-testid="about-check-update"]').trigger('click')
    await flushPromises()
    expect(wrapper.get('[data-testid="about-update-uptodate"]').exists()).toBe(true)
  })

  it('shows available update and can start install', async () => {
    const downloadAndInstall = vi.fn().mockResolvedValue(undefined)
    check.mockResolvedValueOnce({
      version: '0.4.0',
      body: 'fixes',
      downloadAndInstall,
    })
    const wrapper = mount(AboutPage)
    await flushPromises()
    await wrapper.get('[data-testid="about-check-update"]').trigger('click')
    await flushPromises()
    expect(wrapper.get('[data-testid="about-update-available"]').text()).toContain('0.4.0')
    await wrapper.get('[data-testid="about-install-update"]').trigger('click')
    await flushPromises()
    expect(downloadAndInstall).toHaveBeenCalled()
    expect(relaunch).toHaveBeenCalled()
  })

  it('shows a readable error when check fails', async () => {
    check.mockRejectedValueOnce(new Error('network'))
    const wrapper = mount(AboutPage)
    await flushPromises()
    await wrapper.get('[data-testid="about-check-update"]').trigger('click')
    await flushPromises()
    expect(wrapper.get('[data-testid="about-update-error"]').text()).toContain('network')
  })
})
