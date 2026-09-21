<script setup lang="ts">
import { ref, computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { open } from '@tauri-apps/plugin-shell';
import { getVersion } from '@tauri-apps/api/app';
import MirrorPanel from './MirrorPanel.vue';
import SoftwareSettings from './SoftwareSettings.vue';
import { setLocale, getLocale, type SupportedLocale } from '../i18n';
import { useTheme, type ThemeMode, setTheme as setThemeFn } from '../composables/useTheme';

const { t } = useI18n();
const { theme: currentTheme } = useTheme(); // 这会触发 onMounted 初始化

type TabType = 'mirrors' | 'software';

const activeTab = ref<TabType>('mirrors');
const currentLocale = ref<SupportedLocale>(getLocale());
const appVersion = ref('');

const RELEASES_URL = 'https://github.com/jeeinn/php-stack/releases';

getVersion()
  .then((v) => {
    appVersion.value = v;
  })
  .catch(() => {
    appVersion.value = '';
  });

const tabs = [
  { id: 'mirrors' as TabType, labelKey: 'settings.tabs.mirrors', icon: '🌐' },
  { id: 'software' as TabType, labelKey: 'settings.tabs.software', icon: '🔧' },
];

function switchLanguage(locale: SupportedLocale) {
  setLocale(locale);
  currentLocale.value = locale;
}

async function openReleases() {
  await open(RELEASES_URL);
}

// 主题选项（使用计算属性确保响应式）
const themeOptions = computed(() => [
  { value: 'auto' as ThemeMode, label: t('settings.theme.auto'), icon: '💻' },
  { value: 'light' as ThemeMode, label: t('settings.theme.light'), icon: '☀️' },
  { value: 'dark' as ThemeMode, label: t('settings.theme.dark'), icon: '🌙' },
]);
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-200 transition-colors duration-300">
    <!-- 标签页头部 -->
    <div class="flex-shrink-0 border-b border-slate-200 dark:border-slate-800 bg-white/50 dark:bg-slate-900/50 backdrop-blur-sm">
      <div class="px-6 py-3">
        <h1 class="text-xl font-bold mb-4 text-slate-900 dark:text-slate-100">{{ $t('settings.title') }}</h1>
        
        <!-- 标签切换按钮 + 语言切换 -->
        <div class="flex items-center justify-between gap-4">
          <div class="flex gap-2">
            <button
              v-for="tab in tabs"
              :key="tab.id"
              @click="activeTab = tab.id"
              :class="[
                'px-4 py-2 rounded-lg font-medium transition-all duration-200 flex items-center gap-2',
                activeTab === tab.id
                  ? 'bg-blue-600 text-white shadow-lg shadow-blue-600/20'
                  : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700 hover:text-slate-900 dark:hover:text-slate-100'
              ]"
            >
              <span>{{ tab.icon }}</span>
              <span>{{ $t(tab.labelKey) }}</span>
            </button>
          </div>
          
          <!-- Language Switcher & Theme Selector -->
          <div class="flex items-center gap-2">
            <!-- Theme Selector -->
            <div class="flex items-center gap-1 bg-slate-100 dark:bg-slate-800 rounded-lg p-1">
              <button
                v-for="option in themeOptions"
                :key="option.value"
                @click="setThemeFn(option.value)"
                :class="[
                  'px-3 py-1 rounded text-xs font-medium transition-all flex items-center gap-1',
                  currentTheme === option.value
                    ? 'bg-blue-600 text-white'
                    : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200'
                ]"
                :title="option.label"
              >
                <span>{{ option.icon }}</span>
              </button>
            </div>
            
            <!-- Language Switcher -->
            <div class="flex items-center gap-1 bg-slate-100 dark:bg-slate-800 rounded-lg p-1">
              <button
                @click="switchLanguage('zh-CN')"
                :class="[
                  'px-3 py-1 rounded text-xs font-medium transition-all',
                  currentLocale === 'zh-CN'
                    ? 'bg-blue-600 text-white'
                    : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200'
                ]"
              >
                中文
              </button>
              <button
                @click="switchLanguage('en')"
                :class="[
                  'px-3 py-1 rounded text-xs font-medium transition-all',
                  currentLocale === 'en'
                    ? 'bg-blue-600 text-white'
                    : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200'
                ]"
              >
                EN
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 标签页内容 -->
    <div class="flex-1 overflow-y-auto scrollbar-hide">
      <div v-if="activeTab === 'mirrors'" class="p-6">
        <MirrorPanel />
      </div>
      <div v-if="activeTab === 'software'" class="p-6">
        <SoftwareSettings />
      </div>

      <div class="px-6 pb-6">
        <div class="mt-2 p-4 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
          <div class="text-sm text-slate-600 dark:text-slate-400">
            <span class="font-medium text-slate-800 dark:text-slate-200">{{ $t('settings.about.title') }}</span>
            <span v-if="appVersion" class="ml-2 font-mono">v{{ appVersion }}</span>
            <p class="text-xs mt-1 text-slate-500 dark:text-slate-500">{{ $t('settings.about.hint') }}</p>
          </div>
          <button
            type="button"
            class="px-4 py-2 text-sm bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-200 rounded-lg font-medium transition"
            @click="openReleases"
          >
            {{ $t('settings.about.checkUpdate') }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
@reference "tailwindcss";
</style>
