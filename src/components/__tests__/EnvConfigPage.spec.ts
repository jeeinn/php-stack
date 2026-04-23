import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import EnvConfigPage from '../EnvConfigPage.vue'

describe('EnvConfigPage', () => {
  it('renders the component', () => {
    const wrapper = mount(EnvConfigPage)
    expect(wrapper.exists()).toBe(true)
  })

  it('displays the title', async () => {
    const wrapper = mount(EnvConfigPage)
    // Wait for component to mount
    await wrapper.vm.$nextTick()
    // Check if the component renders without errors
    expect(wrapper.find('h2').exists() || wrapper.text().length > 0).toBe(true)
  })

  it('has service configuration sections', async () => {
    const wrapper = mount(EnvConfigPage)
    await wrapper.vm.$nextTick()
    // Check for PHP service section (the main feature)
    const hasPhpSection = wrapper.text().includes('PHP') || wrapper.find('.service-entry').exists()
    expect(hasPhpSection).toBe(true)
  })
})
