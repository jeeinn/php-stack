<script setup lang="ts">
import { ref } from 'vue';
import BackupPage from './BackupPage.vue';
import RestorePage from './RestorePage.vue';

type TabType = 'backup' | 'restore';

const activeTab = ref<TabType>('backup');

const tabs = [
  { id: 'backup' as TabType, label: '环境备份', icon: '💾' },
  { id: 'restore' as TabType, label: '环境恢复', icon: '📥' },
];
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden bg-slate-950 text-slate-200">
    <!-- 标签页头部 -->
    <div class="flex-shrink-0 border-b border-slate-800 bg-slate-900/50 backdrop-blur-sm">
      <div class="px-6 py-3">
        <h1 class="text-xl font-bold mb-4 text-slate-100">环境迁移</h1>
        
        <!-- 标签切换按钮 -->
        <div class="flex gap-2">
          <button
            v-for="tab in tabs"
            :key="tab.id"
            @click="activeTab = tab.id"
            :class="[
              'px-4 py-2 rounded-lg font-medium transition-all duration-200 flex items-center gap-2',
              activeTab === tab.id
                ? 'bg-blue-600 text-white shadow-lg shadow-blue-600/20'
                : 'bg-slate-800 text-slate-300 hover:bg-slate-700 hover:text-slate-100'
            ]"
          >
            <span>{{ tab.icon }}</span>
            <span>{{ tab.label }}</span>
          </button>
        </div>
      </div>
    </div>

    <!-- 标签页内容 -->
    <div class="flex-1 overflow-y-auto scrollbar-hide">
      <!-- 环境备份 -->
      <div v-if="activeTab === 'backup'" class="p-6">
        <BackupPage />
      </div>

      <!-- 环境恢复 -->
      <div v-if="activeTab === 'restore'" class="p-6">
        <RestorePage />
      </div>
    </div>
  </div>
</template>

<style scoped>
@reference "tailwindcss";
</style>
