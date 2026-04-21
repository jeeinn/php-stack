import { ref } from 'vue';

export interface ConfirmOptions {
  title?: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  type?: 'danger' | 'warning' | 'info';
}

interface ConfirmState {
  show: boolean;
  options: ConfirmOptions;
}

const state = ref<ConfirmState>({
  show: false,
  options: {
    title: '确认操作',
    message: '',
    confirmText: '确认',
    cancelText: '取消',
    type: 'warning'
  }
});

let resolveFn: ((value: boolean) => void) | null = null;

// 显示确认对话框并返回 Promise
export function showConfirm(options: ConfirmOptions): Promise<boolean> {
  return new Promise((resolve) => {
    state.value.options = {
      title: options.title || '确认操作',
      message: options.message,
      confirmText: options.confirmText || '确认',
      cancelText: options.cancelText || '取消',
      type: options.type || 'warning'
    };
    resolveFn = resolve;
    state.value.show = true;
  });
}

// 处理确认
export function handleConfirm() {
  state.value.show = false;
  if (resolveFn) {
    resolveFn(true);
    resolveFn = null;
  }
}

// 处理取消
export function handleCancel() {
  state.value.show = false;
  if (resolveFn) {
    resolveFn(false);
    resolveFn = null;
  }
}

// 获取状态（供组件使用）
export function getConfirmState() {
  return state;
}
