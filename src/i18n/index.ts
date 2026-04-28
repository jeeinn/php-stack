import { createI18n } from 'vue-i18n';
import zhCN from './locales/zh-CN.json';
import en from './locales/en.json';

// 调试：检查翻译数据是否正确加载
console.log('[i18n] zhCN.mirror.dockerRegistry:', JSON.stringify(zhCN.mirror?.dockerRegistry, null, 2));
console.log('[i18n] en.mirror.dockerRegistry:', JSON.stringify(en.mirror?.dockerRegistry, null, 2));

export type SupportedLocale = 'zh-CN' | 'en';

const STORAGE_KEY = 'php-stack-locale';

function getDefaultLocale(): SupportedLocale {
  const saved = localStorage.getItem(STORAGE_KEY);
  if (saved === 'zh-CN' || saved === 'en') return saved;

  // 尝试从浏览器语言推断
  const browserLang = navigator.language;
  if (browserLang.startsWith('zh')) return 'zh-CN';
  return 'en';
}

const i18n = createI18n({
  legacy: false,
  locale: getDefaultLocale(),
  fallbackLocale: 'en',
  messages: {
    'zh-CN': zhCN,
    en,
  },
});

/** 切换语言并持久化 */
export function setLocale(locale: SupportedLocale) {
  (i18n.global.locale as any).value = locale;
  localStorage.setItem(STORAGE_KEY, locale);
  document.documentElement.setAttribute('lang', locale);
}

/** 获取当前语言 */
export function getLocale(): SupportedLocale {
  return (i18n.global.locale as any).value;
}

export default i18n;
