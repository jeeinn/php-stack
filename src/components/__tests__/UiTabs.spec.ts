import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import { ref } from 'vue'
import UiTabs from '../UiTabs.vue'

describe('UiTabs', () => {
  it('renders items and marks the active tab', () => {
    const wrapper = mount(UiTabs, {
      props: {
        modelValue: 'a',
        items: [
          { id: 'a', label: 'Alpha' },
          { id: 'b', label: 'Beta' },
        ],
      },
    })

    const tabs = wrapper.findAll('[role="tab"]')
    expect(tabs).toHaveLength(2)
    expect(tabs[0].attributes('aria-selected')).toBe('true')
    expect(tabs[1].attributes('aria-selected')).toBe('false')
  })

  it('emits update:modelValue when an inactive tab is clicked', async () => {
    const wrapper = mount(UiTabs, {
      props: {
        modelValue: 'a',
        items: [
          { id: 'a', label: 'Alpha' },
          { id: 'b', label: 'Beta', badge: '覆' },
        ],
      },
    })

    await wrapper.findAll('[role="tab"]')[1].trigger('click')
    expect(wrapper.emitted('update:modelValue')?.[0]).toEqual(['b'])
    expect(wrapper.text()).toContain('覆')
  })

  it('works with v-model', async () => {
    const Comp = {
      components: { UiTabs },
      setup() {
        const active = ref('a')
        return { active }
      },
      template: `
        <UiTabs
          v-model="active"
          :items="[{ id: 'a', label: 'A' }, { id: 'b', label: 'B' }]"
        />
        <div data-testid="val">{{ active }}</div>
      `,
    }
    const wrapper = mount(Comp)
    await wrapper.findAll('[role="tab"]')[1].trigger('click')
    expect(wrapper.find('[data-testid="val"]').text()).toBe('b')
  })
})
