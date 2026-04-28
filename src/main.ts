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
app.mount('#app')
