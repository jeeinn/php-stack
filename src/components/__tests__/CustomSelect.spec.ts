import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import CustomSelect from '../CustomSelect.vue';

describe('CustomSelect', () => {
  const options = [
    { value: 'option1', label: '选项1' },
    { value: 'option2', label: '选项2' },
    { value: 'option3', label: '选项3' },
  ];

  it('应该正确渲染组件', () => {
    const wrapper = mount(CustomSelect, {
      props: {
        modelValue: '',
        options,
      },
    });

    expect(wrapper.find('button').exists()).toBe(true);
    expect(wrapper.text()).toContain('请选择...');
  });

  it('应该显示选中的值', async () => {
    const wrapper = mount(CustomSelect, {
      props: {
        modelValue: 'option1',
        options,
      },
    });

    expect(wrapper.text()).toContain('选项1');
  });

  it('点击按钮应该打开下拉框', async () => {
    const wrapper = mount(CustomSelect, {
      props: {
        modelValue: '',
        options,
      },
    });

    await wrapper.find('button').trigger('click');
    expect(wrapper.findAll('.absolute div').length).toBeGreaterThan(0);
  });

  it('选择选项应该触发update:modelValue事件', async () => {
    const wrapper = mount(CustomSelect, {
      props: {
        modelValue: '',
        options,
      },
    });

    // 打开下拉框
    await wrapper.find('button').trigger('click');
    // 等待下拉框动画完成
    await new Promise(resolve => setTimeout(resolve, 150));
    
    // 查找所有选项
    const optionElements = wrapper.findAll('[role="option"]');
    expect(optionElements.length).toBeGreaterThan(0);
    
    // 点击第一个选项
    await optionElements[0].trigger('click');
    
    // 等待事件处理
    await wrapper.vm.$nextTick();
    await new Promise(resolve => setTimeout(resolve, 50));
    
    // 检查是否发出了update:modelValue事件（必须触发）
    const emitted = wrapper.emitted('update:modelValue');
    expect(emitted).toBeTruthy();
    expect(emitted?.[0]).toEqual(['option1']);
  });

  it('禁用状态下不能交互', async () => {
    const wrapper = mount(CustomSelect, {
      props: {
        modelValue: '',
        options,
        disabled: true,
      },
    });

    const button = wrapper.find('button');
    expect(button.attributes('disabled')).toBeDefined();
    
    await button.trigger('click');
    expect(wrapper.emitted('update:modelValue')).toBeFalsy();
  });

  it('空选项列表时显示提示信息', async () => {
    const wrapper = mount(CustomSelect, {
      props: {
        modelValue: '',
        options: [],
      },
    });

    await wrapper.find('button').trigger('click');
    expect(wrapper.text()).toContain('暂无选项');
  });

  it('支持不同尺寸', () => {
    const smWrapper = mount(CustomSelect, {
      props: {
        modelValue: '',
        options,
        size: 'sm',
      },
    });

    const mdWrapper = mount(CustomSelect, {
      props: {
        modelValue: '',
        options,
        size: 'md',
      },
    });

    const lgWrapper = mount(CustomSelect, {
      props: {
        modelValue: '',
        options,
        size: 'lg',
      },
    });

    // 检查是否应用了正确的尺寸类
    expect(smWrapper.find('button').classes()).toContain('text-xs');
    expect(mdWrapper.find('button').classes()).toContain('text-sm');
    expect(lgWrapper.find('button').classes()).toContain('text-base');
  });
});
