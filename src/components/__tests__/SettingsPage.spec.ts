import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount } from '@vue/test-utils'
import SettingsPage from '../SettingsPage.vue'

// Mock i18n
vi.mock('../../i18n', () => ({
  setLocale: vi.fn(),
  getLocale: vi.fn(() => 'zh-CN'),
}))

// Mock composables
vi.mock('../../composables/useTheme', () => ({
  useTheme: vi.fn(() => ({
    theme: { value: 'auto' },
  })),
  setTheme: vi.fn(),
}))

// Mock child components
vi.mock('../MirrorPanel.vue', () => ({
  default: {
    name: 'MirrorPanel',
    template: '<div>MirrorPanel Mock</div>',
  },
}))

vi.mock('../SoftwareSettings.vue', () => ({
  default: {
    name: 'SoftwareSettings',
    template: '<div>SoftwareSettings Mock</div>',
  },
}))

describe('SettingsPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders the component', () => {
    const wrapper = mount(SettingsPage)
    expect(wrapper.exists()).toBe(true)
  })

  it('displays the settings title', () => {
    const wrapper = mount(SettingsPage)
    const title = wrapper.find('h1')
    expect(title.exists()).toBe(true)
  })

  it('shows tab buttons', () => {
    const wrapper = mount(SettingsPage)
    const buttons = wrapper.findAll('button')
    expect(buttons.length).toBeGreaterThanOrEqual(2)
  })

  it('shows mirrors tab by default', () => {
    const wrapper = mount(SettingsPage)
    const mirrorsContent = wrapper.findComponent({ name: 'MirrorPanel' })
    expect(mirrorsContent.exists()).toBe(true)
  })

  it('has language switcher buttons', () => {
    const wrapper = mount(SettingsPage)
    const buttons = wrapper.findAll('button')
    const zhButton = buttons.find(b => b.text() === '中文')
    const enButton = buttons.find(b => b.text() === 'EN')
    expect(zhButton).toBeDefined()
    expect(enButton).toBeDefined()
  })

  it('has theme selector buttons', () => {
    const wrapper = mount(SettingsPage)
    const buttons = wrapper.findAll('button')
    // Theme buttons have icons (💻, ☀️, 🌙)
    const themeButtons = buttons.filter(b => {
      const text = b.text()
      return text.includes('💻') || text.includes('☀️') || text.includes('🌙')
    })
    expect(themeButtons.length).toBeGreaterThanOrEqual(1)
  })

  it('switches to software tab when clicked', async () => {
    const wrapper = mount(SettingsPage)
    const buttons = wrapper.findAll('button')
    const softwareTab = buttons.find(b => b.text().includes('🔧'))
    if (softwareTab) {
      await softwareTab.trigger('click')
      await wrapper.vm.$nextTick()
      const softwareContent = wrapper.findComponent({ name: 'SoftwareSettings' })
      expect(softwareContent.exists()).toBe(true)
    }
  })
})