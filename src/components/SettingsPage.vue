<script setup lang="ts">
import { ref, computed } from 'vue';
import { useI18n } from 'vue-i18n';
import MirrorPanel from './MirrorPanel.vue';
import SoftwareSettings from './SoftwareSettings.vue';
import { setLocale, getLocale, type SupportedLocale } from '../i18n';
import { useTheme, type ThemeMode, setTheme as setThemeFn } from '../composables/useTheme';

const { t } = useI18n();
const { theme: currentTheme } = useTheme(); // 这会触发 onMounted 初始化

type TabType = 'mirrors' | 'software';

const activeTab = ref<TabType>('mirrors');
const currentLocale = ref<SupportedLocale>(getLocale());

const tabs = [
  { id: 'mirrors' as TabType, labelKey: 'settings.tabs.mirrors', icon: '🌐' },
  { id: 'software' as TabType, labelKey: 'settings.tabs.software', icon: '🔧' },
];

function switchLanguage(locale: SupportedLocale) {
  setLocale(locale);
  currentLocale.value = locale;
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
    </div>
  </div>
</template>

<style scoped>
@reference "tailwindcss";
</style>
