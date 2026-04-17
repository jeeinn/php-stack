import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue()],
  // 开发服务器配置
  server: {
    // 忽略 docker-compose.yml 等动态生成的文件，避免触发重新构建
    watch: {
      ignored: [
        '**/docker-compose.yml',
        '**/docker-compose.override.yml',
        '**/data/**', // 数据目录
      ]
    }
  }
})
