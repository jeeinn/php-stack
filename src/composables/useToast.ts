import { ref } from 'vue';

export type ToastType = 'success' | 'error' | 'warning' | 'info';

interface ToastItem {
  id: number;
  message: string;
  type: ToastType;
  duration?: number;
}

const toasts = ref<ToastItem[]>([]);
let nextId = 0;

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
