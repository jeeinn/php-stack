// Test setup file for Vue components
import { config } from '@vue/test-utils'
import { createI18n } from 'vue-i18n'
import zhCN from '../i18n/locales/zh-CN.json'
import en from '../i18n/locales/en.json'

// Create a separate i18n instance for tests
const i18n = createI18n({
  legacy: false,
  locale: 'zh-CN',
  fallbackLocale: 'en',
  messages: { 'zh-CN': zhCN, en },
})

// Register vue-i18n plugin globally for all tests
// Use the install function directly as the plugin entry
config.global.plugins.push(i18n as any)

// Mock Tauri API calls
// 守卫原因：setupFiles 对所有测试生效，但 scripts/ 下的脚本单元测试跑在
// node 环境（无 window）。不加守卫会在那里抛 ReferenceError，导致整个套件
// 连文件都加载不了（表现为 "0 test / Failed Suites"）。
if (typeof window !== 'undefined') {
  ;(window as any).invoke = async (command: string, args?: any) => {
    console.log(`Mock invoke called: ${command}`, args)
    switch (command) {
      case 'check_docker':
        return { available: true, version: '20.10.0' }
      case 'list_containers':
        return []
      case 'get_env_config':
        return {}
      default:
        return null
    }
  }
}
