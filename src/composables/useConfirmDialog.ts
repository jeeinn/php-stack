import { ref } from 'vue';
import i18n from '../i18n';

export interface ConfirmOptions {
  title?: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  /** 次级确认按钮文案（显示在主确认按钮左侧） */
  secondaryText?: string;
  type?: 'danger' | 'warning' | 'info';
  checkboxLabel?: string; // 复选框标签
  checkboxDefault?: boolean; // 复选框默认值
}

export interface ConfirmResult {
  confirmed: boolean;
  checkboxValue: boolean;
  /** 是否点击了次级确认按钮（如「应用&启动」） */
  secondary?: boolean;
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

let resolveFn: ((value: boolean | ConfirmResult) => void) | null = null;

function t(key: string): string {
  return String(i18n.global.t(key));
}

// 显示确认对话框并返回 Promise<boolean | ConfirmResult>
export function showConfirm(options: ConfirmOptions): Promise<boolean | ConfirmResult> {
  return new Promise((resolve) => {
    state.value.options = {
      title: options.title || t('common.confirmAction'),
      message: options.message,
      confirmText: options.confirmText || t('common.confirm'),
      cancelText: options.cancelText || t('common.cancel'),
      secondaryText: options.secondaryText,
      type: options.type || 'warning',
      checkboxLabel: options.checkboxLabel,
      checkboxDefault: options.checkboxDefault
    };
    state.value.checkboxValue = options.checkboxDefault || false;
    resolveFn = resolve;
    state.value.show = true;
  });
}

function resolveConfirm(secondary: boolean) {
  state.value.show = false;
  if (!resolveFn) return;

  // 有复选框或次级按钮时返回对象，便于调用方读取 checkbox / secondary
  if (state.value.options.checkboxLabel || state.value.options.secondaryText) {
    resolveFn({
      confirmed: true,
      checkboxValue: state.value.checkboxValue,
      secondary,
    });
  } else {
    resolveFn(true);
  }
  resolveFn = null;
}

// 处理主确认
export function handleConfirm() {
  resolveConfirm(false);
}

// 处理次级确认（如「应用&启动」）
export function handleSecondary() {
  resolveConfirm(true);
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
