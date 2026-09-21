import { ref } from 'vue';
import i18n from '../i18n';

export interface ConfirmOptions {
  title?: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  type?: 'danger' | 'warning' | 'info';
  checkboxLabel?: string; // 复选框标签
  checkboxDefault?: boolean; // 复选框默认值
}

interface ConfirmState {
  show: boolean;
  options: ConfirmOptions;
  checkboxValue: boolean; // 复选框的值
}

const state = ref<ConfirmState>({
  show: false,
  options: {
    title: '',
    message: '',
    confirmText: '',
    cancelText: '',
    type: 'warning'
  },
  checkboxValue: false
});

let resolveFn: ((value: boolean | { confirmed: boolean; checkboxValue: boolean }) => void) | null = null;

function t(key: string): string {
  return String(i18n.global.t(key));
}

// 显示确认对话框并返回 Promise<boolean | { confirmed: boolean; checkboxValue: boolean }>
export function showConfirm(options: ConfirmOptions): Promise<boolean | { confirmed: boolean; checkboxValue: boolean }> {
  return new Promise((resolve) => {
    state.value.options = {
      title: options.title || t('common.confirmAction'),
      message: options.message,
      confirmText: options.confirmText || t('common.confirm'),
      cancelText: options.cancelText || t('common.cancel'),
      type: options.type || 'warning',
      checkboxLabel: options.checkboxLabel,
      checkboxDefault: options.checkboxDefault
    };
    state.value.checkboxValue = options.checkboxDefault || false;
    resolveFn = resolve;
    state.value.show = true;
  });
}

// 处理确认
export function handleConfirm() {
  state.value.show = false;
  if (resolveFn) {
    // 如果有复选框，返回对象；否则返回布尔值
    if (state.value.options.checkboxLabel) {
      resolveFn({ 
        confirmed: true, 
        checkboxValue: state.value.checkboxValue 
      });
    } else {
      resolveFn(true);
    }
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
