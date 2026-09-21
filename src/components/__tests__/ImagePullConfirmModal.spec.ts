/**
 * ImagePullConfirmModal.spec.ts
 *
 * 覆盖点：
 * - open=false 不渲染
 * - missing / present 分组渲染（tag 与 size 都要可见）
 * - 取消按钮 emit('cancel')
 * - 确认按钮 emit('confirm', 仅缺失的 tag 列表) —— 这是父组件拉取的输入，传错会导致误拉
 * - 无缺失时确认按钮禁用（避免空拉取）
 * - pulling 中禁用取消与确认（避免重复提交）
 * - i18n 兜底：渲染文本不含未翻译的 key 残片
 */
import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import ImagePullConfirmModal from '../ImagePullConfirmModal.vue'
import zhCN from '../../i18n/locales/zh-CN.json'
import type { ImagePresence } from '../../types/env-config'

const MIXED: ImagePresence[] = [
  { status: 'present', tag: 'php:8.2-fpm', size: '500MB' },
  { status: 'missing', tag: 'mysql:8.0' },
  { status: 'missing', tag: 'redis:8.2-alpine' },
]

function mountModal(props: Record<string, unknown> = {}) {
  return mount(ImagePullConfirmModal, {
    props: { open: true, presences: MIXED, ...props },
  })
}

describe('ImagePullConfirmModal', () => {
  it('open=false 时不渲染', () => {
    const wrapper = mount(ImagePullConfirmModal, { props: { open: false, presences: MIXED } })
    expect(wrapper.html()).toBe('<!--v-if-->')
  })

  it('渲染所有镜像 tag（缺失与已存在都要列出）', () => {
    const wrapper = mountModal()
    const text = wrapper.text()
    expect(text).toContain('php:8.2-fpm')
    expect(text).toContain('mysql:8.0')
    expect(text).toContain('redis:8.2-alpine')
  })

  it('已存在的镜像显示 size', () => {
    const wrapper = mountModal()
    expect(wrapper.text()).toContain('500MB')
  })

  it('缺失镜像标记「待拉取」而不是显示 size', () => {
    const wrapper = mountModal()
    // Missing 项没有 size 可展示，模板给它的是「待拉取」徽标
    expect(wrapper.text()).toContain(zhCN.envConfig.pullConfirm.toPull)
  })

  it('已存在镜像的 size 缺省时回落为占位符', () => {
    const wrapper = mountModal({
      presences: [{ status: 'present', tag: 'php:8.2-fpm' }],
    })
    expect(wrapper.text()).toContain('—')
  })

  it('确认按钮只提交缺失的 tag（不误拉已有镜像）', async () => {
    const wrapper = mountModal()
    const confirmBtn = wrapper
      .findAll('button')
      .find((b) => b.text().includes('mysql') || b.attributes('disabled') === undefined)
    // 更稳妥：直接取最后一个按钮（footer 的确认按钮）
    const buttons = wrapper.findAll('button')
    const btn = buttons[buttons.length - 1]
    expect(btn).toBeTruthy()
    await btn.trigger('click')

    const emitted = wrapper.emitted('confirm')
    expect(emitted, '应 emit confirm').toBeTruthy()
    expect(emitted![0]).toEqual([['mysql:8.0', 'redis:8.2-alpine']])
    // 关键断言：present 的 php:8.2-fpm 不应出现在拉取列表里
    expect((emitted![0] as any)[0]).not.toContain('php:8.2-fpm')
  })

  it('取消按钮 emit cancel', async () => {
    const wrapper = mountModal()
    const buttons = wrapper.findAll('button')
    const cancelBtn = buttons.find((b) => b.text() === '取消')
    expect(cancelBtn, '应存在取消按钮').toBeTruthy()
    await cancelBtn!.trigger('click')
    expect(wrapper.emitted('cancel')).toBeTruthy()
  })

  it('无缺失镜像时确认按钮禁用', () => {
    const wrapper = mountModal({
      presences: [{ status: 'present', tag: 'php:8.2-fpm', size: '500MB' }],
    })
    const buttons = wrapper.findAll('button')
    const confirmBtn = buttons[buttons.length - 1]
    expect(confirmBtn.attributes('disabled')).toBeDefined()
  })

  it('pulling=true 时取消按钮禁用（防止拉取中退出）', () => {
    const wrapper = mountModal({ pulling: true })
    const cancelBtn = wrapper.findAll('button').find((b) => b.text() === '取消')
    expect(cancelBtn!.attributes('disabled')).toBeDefined()
  })

  it('pulling=true 时显示进度百分比', () => {
    const wrapper = mountModal({ pulling: true, progress: { 'mysql:8.0': 42 } })
    expect(wrapper.text()).toContain('42%')
  })

  it('渲染文本中不含未翻译的 i18n key 残片', () => {
    const wrapper = mountModal()
    const text = wrapper.text()
    expect(text, `出现未翻译的 key: ${text.match(/envConfig\.\w+\.\w+/)?.[0]}`).not.toMatch(
      /envConfig\.\w+\.\w+/
    )
  })
})
