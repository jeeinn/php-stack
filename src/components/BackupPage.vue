<script setup lang="ts">
import { ref, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { save, open } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import type { BackupOptions, BackupProgress } from '../types/env-config';
import { showToast, addLog } from '../composables/useToast';

const options = ref<BackupOptions>({
  include_database: false,
  include_vhosts: false,
  include_projects: false,
  project_patterns: [],
  include_logs: false,
});

const projectPatternsText = ref('');

async function selectProjectFolder() {
  const selected = await open({
    directory: true,
    multiple: false,
    defaultPath: './', // 默认打开当前目录
  });
  if (selected) {
    try {
      const relativePath = await invoke<string>('convert_to_relative_path', { 
        absolutePath: selected,
        isDirectory: true
      });
      appendPattern(relativePath);
    } catch (e) {
      handlePathError(e as string);
    }
  }
}

async function selectProjectFile() {
  const selected = await open({
    directory: false,
    multiple: false,
    defaultPath: './', // 默认打开当前目录
  });
  if (selected) {
    try {
      const relativePath = await invoke<string>('convert_to_relative_path', { 
        absolutePath: selected,
        isDirectory: false
      });
      appendPattern(relativePath);
    } catch (e) {
      handlePathError(e as string);
    }
  }
}

function handlePathError(errorMsg: string) {
  console.error('[Backup] Path conversion failed:', errorMsg);
  addLog(`路径转换失败: ${errorMsg}`);
  showToast(errorMsg, 'error');
}

function appendPattern(pattern: string) {
  const current = projectPatternsText.value.trim();
  if (current) {
    // 避免重复添加
    if (!current.split('\n').includes(pattern)) {
      projectPatternsText.value = `${current}\n${pattern}`;
    }
  } else {
    projectPatternsText.value = pattern;
  }
}
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
  // Generate filename with format: YYYYMMDD-HHIISS
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, '0');
  const day = String(now.getDate()).padStart(2, '0');
  const hours = String(now.getHours()).padStart(2, '0');
  const minutes = String(now.getMinutes()).padStart(2, '0');
  const seconds = String(now.getSeconds()).padStart(2, '0');
  const timestamp = `${year}${month}${day}-${hours}${minutes}${seconds}`;
  
  // Select save path
  const savePath = await save({
    filters: [{ name: 'PHP-Stack Backup', extensions: ['zip'] }],
    defaultPath: `php-stack-backup-${timestamp}.zip`,
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

          <!-- Projects -->
          <div>
            <label class="flex items-center gap-3 text-sm text-slate-300 cursor-pointer">
              <input type="checkbox" v-model="options.include_projects" class="accent-blue-500" />
              <span>包含项目文件</span>
            </label>
            <transition name="fade">
              <div v-if="options.include_projects" class="mt-3 ml-7">
                <div class="flex items-center justify-between mb-1">
                  <label class="block text-xs text-slate-400">文件匹配模式（每行一个 glob 模式）</label>
                  <div class="flex gap-2">
                    <button 
                      @click="selectProjectFolder"
                      class="text-xs px-2 py-1 bg-blue-600/20 text-blue-400 rounded hover:bg-blue-600 hover:text-white transition"
                    >
                      选择文件夹
                    </button>
                    <button 
                      @click="selectProjectFile"
                      class="text-xs px-2 py-1 bg-emerald-600/20 text-emerald-400 rounded hover:bg-emerald-600 hover:text-white transition"
                    >
                      选择文件
                    </button>
                  </div>
                </div>
                <textarea
                  v-model="projectPatternsText"
                  placeholder="例如：&#10;www/project-a/**&#10;.env"
                  class="w-full h-24 bg-slate-800 border border-slate-700 rounded-lg p-3 text-xs font-mono text-blue-300 focus:ring-1 focus:ring-blue-500 outline-none"
                ></textarea>
                <p class="text-[10px] text-slate-500 mt-1">
                  💡 提示：为确保跨平台恢复成功，请仅备份项目目录内的文件。
                </p>
              </div>
            </transition>
          </div>

          <!-- Logs -->
          <label class="flex items-center gap-3 text-sm text-slate-300 cursor-pointer">
            <input type="checkbox" v-model="options.include_logs" class="accent-blue-500" />
            <div class="flex flex-col">
              <span>包含日志文件（logs 目录）</span>
              <span class="text-xs text-slate-500">注意：日志文件可能占用较大备份空间</span>
            </div>
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
