import { ref } from 'vue';

export type ToastType = 'success' | 'error' | 'warning' | 'info';

interface ToastItem {
  id: number;
  message: string;
  type: ToastType;
  duration?: number;
}

/** UI 日志面板保留条数；超出丢弃最早的。完整排查请用「导出」。 */
export const UI_LOG_LIMIT = 200;

const toasts = ref<ToastItem[]>([]);
const logs = ref<string[]>([]);
let nextId = 0;

// 添加日志（用于实时日志面板）
export function addLog(message: string) {
  const time = new Date().toLocaleTimeString();
  logs.value.push(`[${time}] ${message}`);
  while (logs.value.length > UI_LOG_LIMIT) {
    logs.value.shift();
  }
}

// 获取当前所有日志
export function getLogs() {
  return logs;
}

// 清空日志面板
export function clearLogs() {
  logs.value = [];
}

// 显示 Toast
export function showToast(message: string, type: ToastType = 'info', duration = 3000) {
  const id = nextId++;
  toasts.value.push({ id, message, type, duration });
  
  // 自动移除
  if (duration > 0) {
    setTimeout(() => {
      removeToast(id);
    }, duration);
  }
}

// 移除 Toast
export function removeToast(id: number) {
  const index = toasts.value.findIndex(t => t.id === id);
  if (index > -1) {
    toasts.value.splice(index, 1);
  }
}

// 获取当前所有 Toast（供组件使用）
export function getToasts() {
  return toasts;
}
