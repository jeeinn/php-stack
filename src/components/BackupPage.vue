<script setup lang="ts">
import { ref, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import type { BackupOptions, BackupProgress } from '../types/env-config';
import { showToast } from '../composables/useToast';

const options = ref<BackupOptions>({
  include_database: false,
  include_projects: false,
  project_patterns: [],
  include_vhosts: false,
  include_logs: false,
});

const projectPatternsText = ref('.env\nsrc/config/*.php');
const backing = ref(false);
const progress = ref<BackupProgress | null>(null);

// Listen for backup progress events
let unlisten: (() => void) | null = null;

async function setupListener() {
  unlisten = await listen<BackupProgress>('backup-progress', (event) => {
    progress.value = event.payload;
  });
}
setupListener();

onUnmounted(() => {
  if (unlisten) unlisten();
});

async function handleBackup() {
  // Select save path
  const savePath = await save({
    filters: [{ name: 'PHP-Stack Backup', extensions: ['zip'] }],
    defaultPath: `php-stack-backup-${new Date().toISOString().slice(0, 10)}.zip`,
  });

  if (!savePath) return;

  backing.value = true;
  progress.value = { step: '准备中...', percentage: 0 };

  try {
    // Build options
    const backupOptions: BackupOptions = {
      ...options.value,
      project_patterns: options.value.include_projects
        ? projectPatternsText.value.split('\n').map(l => l.trim()).filter(Boolean)
        : [],
    };

    await invoke('create_backup', { savePath, options: backupOptions });
    showToast(`备份已成功创建：${savePath}`, 'success');
    progress.value = { step: '完成', percentage: 100 };
  } catch (e) {
    showToast(e as string, 'error');
  } finally {
    backing.value = false;
  }
}
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden">
    <header class="mb-6">
      <h1 class="text-3xl font-bold">环境备份</h1>
      <p class="text-slate-400 text-sm mt-1">一键导出当前开发环境的完整备份</p>
    </header>

    <div class="flex-1 overflow-y-auto pr-2 space-y-6">
      <!-- Backup Options -->
      <section class="bg-slate-900 border border-slate-800 rounded-xl p-6">
        <h2 class="text-lg font-bold mb-4">📋 备份选项</h2>
        <div class="space-y-4">
          <!-- Always included -->
          <label class="flex items-center gap-3 text-sm text-slate-400">
            <input type="checkbox" checked disabled class="accent-blue-500" />
            <span>核心配置文件（.env、docker-compose.yml、services/ 配置）</span>
          </label>

          <!-- Database -->
          <label class="flex items-center gap-3 text-sm text-slate-300 cursor-pointer">
            <input type="checkbox" v-model="options.include_database" class="accent-blue-500" />
            <span>包含数据库数据（MySQL dump）</span>
          </label>

          <!-- Projects -->
          <div>
            <label class="flex items-center gap-3 text-sm text-slate-300 cursor-pointer">
              <input type="checkbox" v-model="options.include_projects" class="accent-blue-500" />
              <span>包含项目文件</span>
            </label>
            <transition name="fade">
              <div v-if="options.include_projects" class="mt-3 ml-7">
                <label class="block text-xs text-slate-400 mb-1">文件匹配模式（每行一个 glob 模式）</label>
                <textarea
                  v-model="projectPatternsText"
                  placeholder="每行一个路径模式，如：&#10;.env&#10;src/config/*.php"
                  class="w-full h-24 bg-slate-800 border border-slate-700 rounded-lg p-3 text-xs font-mono text-blue-300 focus:ring-1 focus:ring-blue-500 outline-none"
                ></textarea>
              </div>
            </transition>
          </div>

          <!-- Vhosts -->
          <label class="flex items-center gap-3 text-sm text-slate-300 cursor-pointer">
            <input type="checkbox" v-model="options.include_vhosts" class="accent-blue-500" />
            <span>包含 Nginx 虚拟主机配置</span>
          </label>

          <!-- Logs -->
          <label class="flex items-center gap-3 text-sm text-slate-300 cursor-pointer">
            <input type="checkbox" v-model="options.include_logs" class="accent-blue-500" />
            <span>包含日志文件（最近 7 天）</span>
          </label>
        </div>
      </section>

      <!-- Progress -->
      <section v-if="progress" class="bg-slate-900 border border-slate-800 rounded-xl p-6">
        <h2 class="text-lg font-bold mb-4">📊 备份进度</h2>
        <div class="mb-2 text-sm text-slate-300">{{ progress.step }}</div>
        <div class="w-full bg-slate-800 rounded-full h-3 overflow-hidden">
          <div
            class="h-full bg-blue-600 rounded-full transition-all duration-300"
            :style="{ width: progress.percentage + '%' }"
          ></div>
        </div>
        <div class="text-xs text-slate-500 mt-1 text-right">{{ progress.percentage }}%</div>
      </section>

      <!-- Action -->
      <section class="bg-slate-900 border border-slate-800 rounded-xl p-6">
        <button
          @click="handleBackup"
          :disabled="backing"
          class="w-full py-3 bg-blue-600 hover:bg-blue-700 rounded-xl font-bold transition disabled:opacity-50 flex items-center justify-center gap-2"
        >
          <span v-if="backing" class="inline-block animate-spin rounded-full h-4 w-4 border-b-2 border-white"></span>
          {{ backing ? '备份中...' : '创建备份包 (.zip)' }}
        </button>
      </section>
    </div>
  </div>
</template>
