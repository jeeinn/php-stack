import { createApp } from 'vue'
import './style.css'
import App from './App.vue'
import i18n from './i18n'
import { setTheme, applyTheme } from './composables/useTheme'

// 初始化主题（在 mount 之前，直接应用保存的主题）
const savedTheme = localStorage.getItem('php-stack-theme')
if (savedTheme && ['light', 'dark', 'auto'].includes(savedTheme)) {
  setTheme(savedTheme as 'light' | 'dark' | 'auto')
} else {
  // 默认使用 auto 模式
  setTheme('auto')
}

// 立即应用主题
applyTheme()

const app = createApp(App)
app.use(i18n)

// 兜底：Vue 内部的渲染/生命周期错误不会触发 window.onerror，走 errorHandler 上报
app.config.errorHandler = (err: unknown, _instance: unknown, info: string) => {
  const guard = (
    window as unknown as {
      __phpStackBootGuard?: { report: (raw: Record<string, unknown>) => void }
    }
  ).__phpStackBootGuard
  guard?.report({
    message: err instanceof Error ? err.message : String(err),
    source: `vue:${info}`,
    stack: err instanceof Error ? err.stack : undefined,
  })
  console.error(err)
}

app.mount('#app')

// 挂载成功后取消白屏兜底面板；之后的错误只上报，不再覆盖界面
;(
  window as unknown as { __phpStackBootGuard?: { markMounted: () => void } }
).__phpStackBootGuard?.markMounted()
