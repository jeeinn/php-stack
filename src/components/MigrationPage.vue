<script setup lang="ts">
import { ref } from 'vue';
import { useI18n } from 'vue-i18n';
import BackupPage from './BackupPage.vue';
import RestorePage from './RestorePage.vue';

const { t } = useI18n();

type TabType = 'backup' | 'restore';

const activeTab = ref<TabType>('backup');

const tabs = [
  { id: 'backup' as TabType, labelKey: 'migration.tabs.backup', icon: '💾' },
  { id: 'restore' as TabType, labelKey: 'migration.tabs.restore', icon: '⬇️' },
];
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden bg-slate-950 text-slate-200">
    <!-- 标签页头部 -->
    <div class="flex-shrink-0 border-b border-slate-800 bg-slate-900/50 backdrop-blur-sm">
      <div class="px-4 sm:px-6 py-3">
        <h1 class="text-xl sm:text-2xl font-bold mb-3 sm:mb-4 text-slate-100">{{ $t('migration.title') }}</h1>
        
        <!-- 标签切换按钮 -->
        <div class="flex flex-col sm:flex-row gap-2">
          <button
            v-for="tab in tabs"
            :key="tab.id"
            @click="activeTab = tab.id"
            :class="[
              'w-full sm:w-auto px-4 py-2 rounded-lg font-medium transition-all duration-200 flex items-center justify-center sm:justify-start gap-2',
              activeTab === tab.id
                ? 'bg-blue-600 text-white shadow-lg shadow-blue-600/20'
                : 'bg-slate-800 text-slate-300 hover:bg-slate-700 hover:text-slate-100'
            ]"
          >
            <span>{{ tab.icon }}</span>
            <span>{{ $t(tab.labelKey) }}</span>
          </button>
        </div>
      </div>
    </div>

    <!-- 标签页内容 -->
    <div class="flex-1 overflow-y-auto scrollbar-hide">
      <div v-if="activeTab === 'backup'" class="p-3 sm:p-6">
        <BackupPage />
      </div>
      <div v-if="activeTab === 'restore'" class="p-3 sm:p-6">
        <RestorePage />
      </div>
    </div>
  </div>
</template>

<style scoped>
@reference "tailwindcss";
</style>
