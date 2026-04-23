import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import MirrorPanel from '../MirrorPanel.vue'

describe('MirrorPanel', () => {
  it('renders the component', () => {
    const wrapper = mount(MirrorPanel)
    expect(wrapper.exists()).toBe(true)
  })

  it('displays mirror configuration options', async () => {
    const wrapper = mount(MirrorPanel)
    await wrapper.vm.$nextTick()
    // Check if mirror options are displayed (look for common mirror-related text)
    const hasMirrorContent = wrapper.text().includes('镜像') || 
                             wrapper.text().includes('Mirror') ||
                             wrapper.find('.mirror-option').exists()
    expect(hasMirrorContent).toBe(true)
  })
})
