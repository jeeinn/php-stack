<script setup lang="ts">
import { ref, computed } from 'vue';
import { useI18n } from 'vue-i18n';
import MirrorPanel from './MirrorPanel.vue';
import SoftwareSettings from './SoftwareSettings.vue';
import UiTabs from './UiTabs.vue';
import { setLocale, getLocale, type SupportedLocale } from '../i18n';
import { useTheme, type ThemeMode, setTheme as setThemeFn } from '../composables/useTheme';

const { t } = useI18n();
const { theme: currentTheme } = useTheme(); // 这会触发 onMounted 初始化

type TabType = 'mirrors' | 'software';

const activeTab = ref<TabType>('mirrors');
const currentLocale = ref<SupportedLocale>(getLocale());

const tabItems = computed(() => [
  { id: 'mirrors', label: t('settings.tabs.mirrors') },
  { id: 'software', label: t('settings.tabs.software') },
]);

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
    <!-- 页头：标题 + 主题/语言（分段控件，不是页面 Tab） -->
    <div class="flex-shrink-0 bg-white/50 dark:bg-slate-900/50 backdrop-blur-sm">
      <div class="px-4 sm:px-6 pt-3 sm:pt-4 flex items-start justify-between gap-4">
        <h1 class="text-xl sm:text-2xl font-bold text-slate-900 dark:text-slate-100">{{ $t('settings.title') }}</h1>

        <div class="flex items-center gap-2 shrink-0">
          <div class="flex items-center gap-1 bg-slate-100 dark:bg-slate-800 rounded-lg p-1">
            <button
              v-for="option in themeOptions"
              :key="option.value"
              type="button"
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

          <div class="flex items-center gap-1 bg-slate-100 dark:bg-slate-800 rounded-lg p-1">
            <button
              type="button"
              @click="switchLanguage('zh-CN')"
              :class="[
                'px-3 py-1 rounded text-xs font-medium transition-all',
                currentLocale === 'zh-CN'
                  ? 'bg-blue-600 text-white'
                  : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200'
              ]"
            >
              中文<!-- i18n-exempt: 语言自称，任何界面语言下都显示为「中文」，不随 locale 切换 -->
            </button>
            <button
              type="button"
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

      <div class="px-4 sm:px-6 mt-2">
        <UiTabs v-model="activeTab" :items="tabItems" size="md" />
      </div>
    </div>

    <!-- 标签页内容 -->
    <div class="flex-1 overflow-y-auto scrollbar-hide">
      <div v-if="activeTab === 'mirrors'" class="p-4 sm:p-6">
        <MirrorPanel />
      </div>
      <div v-if="activeTab === 'software'" class="p-4 sm:p-6">
        <SoftwareSettings />
      </div>
    </div>
  </div>
</template>
