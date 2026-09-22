import { computed, ref } from 'vue';
import i18n from '../i18n';

/**
 * Toast 只有三类视觉：成功=绿、错误=红、其余=蓝。
 *
 * 原本还有 'warning'，但它在 Toast.vue 里与 'info' 返回**完全相同**的配色与边框
 * （统一配色时把 amber 改成了蓝），于是「类型名有四个、视觉只有三种」——
 * 调用方以为传 'warning' 会更醒目，界面上却没有任何区别。
 * 与其留一个名不副实的类型，不如把类型体系收敛到与实际视觉一致的三类：
 * 需要强调的内容请写进文案（本项目已这么做，例如「恢复部分失败」会带明细）。
 *
 * 注意与 LogLevel 区分：日志面板的 warn 级别仍在（文字为 amber，且可过滤），
 * 那是**日志**的语义，本轮未改动。
 */
export type ToastType = 'success' | 'error' | 'info';
export type LogLevel = 'info' | 'warn' | 'error';

export interface LogEntry {
  time: string;
  level: LogLevel;
  message: string;
}

interface ToastItem {
  id: number;
  message: string;
  type: ToastType;
  duration?: number;
}

/** UI 日志面板保留条数；超出丢弃最早的。完整排查请用「导出」。 */
export const UI_LOG_LIMIT = 200;

export const LOG_LEVEL_STORAGE_KEY = 'php-stack-log-level';

export const LOG_LEVEL_RANK: Record<LogLevel, number> = {
  info: 0,
  warn: 1,
  error: 2,
};

export function parseLogLevel(value: string | null | undefined): LogLevel {
  if (value === 'warn' || value === 'error') return value;
  return 'info';
}

export function formatLogLine(entry: LogEntry): string {
  return `[${entry.time}] ${entry.level.toUpperCase()} ${entry.message}`;
}

const toasts = ref<ToastItem[]>([]);
const logs = ref<LogEntry[]>([]);
const minLogLevel = ref<LogLevel>(
  typeof localStorage === 'undefined'
    ? 'info'
    : parseLogLevel(localStorage.getItem(LOG_LEVEL_STORAGE_KEY)),
);
let nextId = 0;

export const visibleLogs = computed(() =>
  logs.value.filter(
    (entry) => LOG_LEVEL_RANK[entry.level] >= LOG_LEVEL_RANK[minLogLevel.value],
  ),
);

export function addLog(message: string, level: LogLevel = 'info') {
  const time = new Date().toLocaleTimeString();
  logs.value.push({ time, level, message });
  while (logs.value.length > UI_LOG_LIMIT) {
    logs.value.shift();
  }
}

/** Append a log line using the English locale, independent of the UI language. */
export function addLogKey(
  key: string,
  named?: Record<string, unknown>,
  level: LogLevel = 'info',
) {
  const msg = named
    ? String(i18n.global.t(key, 'en', named))
    : String(i18n.global.t(key, 'en'));
  addLog(msg, level);
}

export function getLogs() {
  return logs;
}

export function getMinLogLevel() {
  return minLogLevel;
}

export function setMinLogLevel(level: LogLevel) {
  minLogLevel.value = level;
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem(LOG_LEVEL_STORAGE_KEY, level);
  }
}

export function clearLogs() {
  logs.value = [];
}

export function showToast(message: string, type: ToastType = 'info', duration = 3000) {
  const id = nextId++;
  toasts.value.push({ id, message, type, duration });

  if (duration > 0) {
    setTimeout(() => {
      removeToast(id);
    }, duration);
  }
}

export function removeToast(id: number) {
  const index = toasts.value.findIndex((t) => t.id === id);
  if (index > -1) {
    toasts.value.splice(index, 1);
  }
}

export function getToasts() {
  return toasts;
}
