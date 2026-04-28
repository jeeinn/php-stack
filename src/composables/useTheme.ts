import { ref, watchEffect } from 'vue';

export type ThemeMode = 'light' | 'dark' | 'auto';

const theme = ref<ThemeMode>('auto');
const systemDarkMode = ref(false);

// 初始化系统主题监听
const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
const updateSystemDarkMode = () => {
  systemDarkMode.value = mediaQuery.matches;
};
updateSystemDarkMode();
mediaQuery.addEventListener('change', updateSystemDarkMode);

// 应用主题到 DOM
export function applyTheme() {
  const html = document.documentElement;
  const mode = theme.value;
  
  console.log('[Theme] applyTheme:', mode, 'systemDarkMode:', systemDarkMode.value);
  
  if (mode === 'auto') {
    // 使用系统主题
    html.classList.toggle('dark', systemDarkMode.value);
    html.classList.toggle('light', !systemDarkMode.value);
  } else {
    html.classList.toggle('dark', mode === 'dark');
    html.classList.toggle('light', mode === 'light');
  }
  
  console.log('[Theme] HTML classes:', html.className);
  
  // 保存到 localStorage
  localStorage.setItem('php-stack-theme', mode);
}

// 设置主题
export function setTheme(newTheme: ThemeMode) {
  console.log('[Theme] setTheme called:', newTheme);
  theme.value = newTheme;
  applyTheme();
}

// 获取当前主题
export function getTheme() {
  return theme.value;
}

// 使用主题的 composable（用于组件中获取响应式状态）
export function useTheme() {
  // 在组件内监听主题和系统主题变化
  watchEffect(() => {
    applyTheme();
  });
  
  return {
    theme,
    setTheme,
    getTheme
  };
}
