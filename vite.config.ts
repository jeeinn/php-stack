import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue()],
  // 开发服务器配置
  server: {
    // 忽略动态生成的文件，避免触发重新构建
    watch: {
      ignored: [
        '**/.env', // 环境配置文件
        '**/docker-compose.yml',
        '**/docker-compose.override.yml',
        '**/data/**', // 数据目录
        '**/logs/**', // 日志目录
        '**/services/**', // 服务配置目录
      ]
    }
  }
})
