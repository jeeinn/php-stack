/**
 * VersionHelpModal.spec.ts
 *
 * 覆盖点：
 * - open=false 时不渲染（避免弹窗常驻 DOM）
 * - open=true 时渲染核心章节
 * - 关闭交互（按钮 + 遮罩）都能 emit('close')
 * - i18n 兜底：任何缺失的 key 都会让 vue-i18n 原样输出 "envConfig.versionHelp.xxx"，
 *   这里断言渲染文本里不含未翻译的 key 残片
 */
import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import VersionHelpModal from '../VersionHelpModal.vue'
import zhCN from '../../i18n/locales/zh-CN.json'
import en from '../../i18n/locales/en.json'

function getByPath(obj: any, path: string): unknown {
  return path.split('.').reduce((acc, k) => (acc == null ? acc : acc[k]), obj)
}

describe('VersionHelpModal', () => {
  it('open=false 时不渲染任何内容', () => {
    const wrapper = mount(VersionHelpModal, { props: { open: false } })
    expect(wrapper.html()).toBe('<!--v-if-->')
  })

  it('open=true 时渲染标题与说明', () => {
    const wrapper = mount(VersionHelpModal, { props: { open: true } })
    expect(wrapper.text()).toContain(zhCN.envConfig.versionHelp.title)
    expect(wrapper.text()).toContain(zhCN.envConfig.versionHelp.subtitle)
  })

  it('渲染命名约定表格的关键示例行（mysql84）', () => {
    const wrapper = mount(VersionHelpModal, { props: { open: true } })
    // 命名约定是本文档的核心契约，示例行必须可见
    expect(wrapper.text()).toContain('mysql84')
    expect(wrapper.text()).toContain('php85')
  })

  it('点击右上角 × 触发 close', async () => {
    const wrapper = mount(VersionHelpModal, { props: { open: true } })
    const closeBtn = wrapper.findAll('button').find((b) => b.text() === '×')
    expect(closeBtn, '应存在关闭按钮').toBeTruthy()
    await closeBtn!.trigger('click')
    expect(wrapper.emitted('close')).toBeTruthy()
  })

  it('点击遮罩触发 close', async () => {
    const wrapper = mount(VersionHelpModal, { props: { open: true } })
    await wrapper.find('div.fixed').trigger('click')
    expect(wrapper.emitted('close')).toBeTruthy()
  })

  it('渲染文本中不含未翻译的 i18n key 残片', () => {
    const wrapper = mount(VersionHelpModal, { props: { open: true } })
    const text = wrapper.text()
    expect(text, `出现未翻译的 key: ${text.match(/envConfig\.\w+\.\w+/)?.[0]}`).not.toMatch(
      /envConfig\.\w+\.\w+/
    )
  })

  // ─── 回归：i18n 消息里的字面花括号 ──────────────────────────
  // vue-i18n 把消息中的 `{...}` 当插值占位符解析。JSON 示例含大量花括号，
  // 曾导致渲染残缺（报 "Invalid token in placeholder"）。示例已改为组件内常量，
  // 这里锁住「花括号必须原样出现在页面上」。
  it('MySQL 5.6 示例 JSON 的花括号完整渲染', () => {
    const wrapper = mount(VersionHelpModal, { props: { open: true } })
    const text = wrapper.text()
    expect(text).toContain('"mysql56"')
    expect(text).toContain('{ "display_name": "MySQL 5.6"')
    expect(text).toContain('"service_dir": "mysql80"')
    expect(text).toContain('"eol": true')
  })

  it('Nginx 示例 JSON 的花括号完整渲染', () => {
    const wrapper = mount(VersionHelpModal, { props: { open: true } })
    const text = wrapper.text()
    expect(text).toContain('"nginx129"')
    expect(text).toContain('"service_dir": "nginx128"')
  })

  it('命名约定说明里的占位符字面量 {PREFIX}/{dir} 不被插值吃掉', () => {
    const wrapper = mount(VersionHelpModal, { props: { open: true } })
    const text = wrapper.text()
    expect(text, '{PREFIX} 应原样显示').toContain('{PREFIX}')
    expect(text, 'ps-{dir} 应原样显示').toContain('ps-{dir}')
  })

  it('中英文文案键一一对应（避免只改了一份 locale）', () => {
    const zhSection = getByPath(zhCN, 'envConfig.versionHelp')
    const enSection = getByPath(en, 'envConfig.versionHelp')
    expect(zhSection, 'zh-CN 缺少 envConfig.versionHelp').toBeTruthy()
    expect(enSection, 'en 缺少 envConfig.versionHelp').toBeTruthy()

    const zhKeys = Object.keys(zhSection as object).sort()
    const enKeys = Object.keys(enSection as object).sort()
    expect(enKeys, 'en 的 versionHelp 键集应与 zh-CN 一致').toEqual(zhKeys)
  })
})
