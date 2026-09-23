<script setup lang="ts">
import { ref, computed } from 'vue';
import { useI18n } from 'vue-i18n';
import BackupPage from './BackupPage.vue';
import RestorePage from './RestorePage.vue';
import UiTabs from './UiTabs.vue';

const { t } = useI18n();

type TabType = 'backup' | 'restore';

const activeTab = ref<TabType>('backup');

const tabItems = computed(() => [
  { id: 'backup', label: t('migration.tabs.backup') },
  { id: 'restore', label: t('migration.tabs.restore') },
]);
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-200 transition-colors duration-300">
    <div class="flex-shrink-0 bg-white/50 dark:bg-slate-900/50 backdrop-blur-sm">
      <div class="px-4 sm:px-6 pt-3 sm:pt-4">
        <h1 class="text-xl sm:text-2xl font-bold text-slate-900 dark:text-slate-100">{{ $t('migration.title') }}</h1>
      </div>
      <div class="px-4 sm:px-6 mt-2">
        <UiTabs v-model="activeTab" :items="tabItems" size="md" />
      </div>
    </div>

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
