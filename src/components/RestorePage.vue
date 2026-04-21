<script setup lang="ts">
import { ref, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import type { RestorePreview, RestoreProgress, PortConflict } from '../types/env-config';
import { showToast } from '../composables/useToast';

const zipPath = ref('');
const preview = ref<RestorePreview | null>(null);
const verified = ref<boolean | null>(null);
const restoring = ref(false);
const loading = ref(false);
const progress = ref<RestoreProgress | null>(null);

// Port overrides for conflicts
const portOverrides = ref<Record<string, number>>({});

// Listen for restore progress events
let unlisten: (() => void) | null = null;

async function setupListener() {
  unlisten = await listen<RestoreProgress>('restore-progress', (event) => {
    progress.value = event.payload;
  });
}
setupListener();

onUnmounted(() => {
  if (unlisten) unlisten();
});

async function selectFile() {
  const selected = await open({
    filters: [{ name: 'PHP-Stack Backup', extensions: ['zip'] }],
    multiple: false,
  });
  if (selected) {
    zipPath.value = selected as string;
    preview.value = null;
    verified.value = null;
    portOverrides.value = {};
  }
}

async function handlePreview() {
  if (!zipPath.value) return;
  loading.value = true;
  try {
    preview.value = await invoke<RestorePreview>('preview_restore', { zipPath: zipPath.value });
    // Initialize port overrides from conflicts
    if (preview.value.port_conflicts.length > 0) {
      for (const conflict of preview.value.port_conflicts) {
        portOverrides.value[conflict.service] = conflict.suggested_port;
      }
    }
  } catch (e) {
    showToast(e as string, 'error');
  } finally {
    loading.value = false;
  }
}

async function handleVerify() {
  if (!zipPath.value) return;
  loading.value = true;
  try {
    verified.value = await invoke<boolean>('verify_backup', { zipPath: zipPath.value });
  } catch (e) {
    showToast(e as string, 'error');
    verified.value = false;
  } finally {
    loading.value = false;
  }
}

async function handleRestore() {
  if (!zipPath.value) return;
  restoring.value = true;
  progress.value = { step: '准备中...', percentage: 0 };

  try {
    await invoke('execute_restore', {
      zipPath: zipPath.value,
      portOverrides: portOverrides.value,
    });
    showToast('环境恢复成功！', 'success');
    progress.value = { step: '完成', percentage: 100 };
  } catch (e) {
    showToast(e as string, 'error');
  } finally {
    restoring.value = false;
  }
}

function applyAutoAssign(conflicts: PortConflict[]) {
  for (const c of conflicts) {
    portOverrides.value[c.service] = c.suggested_port;
  }
}

function formatTimestamp(ts: string): string {
  try {
    return new Date(ts).toLocaleString('zh-CN');
  } catch {
    return ts;
  }
}
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden">
    <header class="mb-6">
      <h1 class="text-3xl font-bold">环境恢复</h1>
      <p class="text-slate-400 text-sm mt-1">从备份包恢复完整开发环境</p>
    </header>

    <div class="flex-1 overflow-y-auto pr-2 space-y-6">
      <!-- File Selection -->
      <section class="bg-slate-900 border border-slate-800 rounded-xl p-6">
        <h2 class="text-lg font-bold mb-4">📁 选择备份文件</h2>
        <div class="flex gap-3">
          <input
            :value="zipPath"
            readonly
            placeholder="请选择 .zip 备份文件"
            class="flex-1 bg-slate-800 border border-slate-700 rounded-lg px-4 py-2 text-sm outline-none text-slate-400"
          />
          <button
            @click="selectFile"
            class="px-5 py-2 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg font-medium transition"
          >
            选择文件
          </button>
        </div>
        <div v-if="zipPath" class="mt-3 flex gap-3">
          <button
            @click="handlePreview"
            :disabled="loading"
            class="px-4 py-2 bg-blue-600/20 text-blue-400 border border-blue-600/30 rounded-lg text-sm hover:bg-blue-600 hover:text-white transition disabled:opacity-50"
          >
            {{ loading ? '加载中...' : '预览内容' }}
          </button>
          <button
            @click="handleVerify"
            :disabled="loading"
            class="px-4 py-2 bg-emerald-600/20 text-emerald-400 border border-emerald-600/30 rounded-lg text-sm hover:bg-emerald-600 hover:text-white transition disabled:opacity-50"
          >
            校验完整性
          </button>
        </div>
      </section>

      <!-- Verification Result -->
      <section v-if="verified !== null" class="bg-slate-900 border border-slate-800 rounded-xl p-6">
        <h2 class="text-lg font-bold mb-3">🔒 完整性校验</h2>
        <div v-if="verified" class="flex items-center gap-2 text-emerald-400">
          <span class="w-3 h-3 bg-emerald-500 rounded-full"></span>
          <span>SHA256 校验通过，备份包完整无损</span>
        </div>
        <div v-else class="flex items-center gap-2 text-rose-400">
          <span class="w-3 h-3 bg-rose-500 rounded-full"></span>
          <span>校验失败，备份包可能已损坏或被篡改</span>
        </div>
      </section>

      <!-- Preview -->
      <section v-if="preview" class="bg-slate-900 border border-slate-800 rounded-xl p-6">
        <h2 class="text-lg font-bold mb-4">📋 备份内容摘要</h2>

        <div class="grid grid-cols-2 gap-4 mb-4">
          <div class="p-3 bg-slate-800/50 border border-slate-700 rounded-lg">
            <div class="text-xs text-slate-400">备份时间</div>
            <div class="text-sm font-medium mt-1">{{ formatTimestamp(preview.manifest.timestamp) }}</div>
          </div>
          <div class="p-3 bg-slate-800/50 border border-slate-700 rounded-lg">
            <div class="text-xs text-slate-400">应用版本</div>
            <div class="text-sm font-medium mt-1">{{ preview.manifest.app_version }}</div>
          </div>
          <div class="p-3 bg-slate-800/50 border border-slate-700 rounded-lg">
            <div class="text-xs text-slate-400">操作系统</div>
            <div class="text-sm font-medium mt-1">{{ preview.manifest.os_info }}</div>
          </div>
          <div class="p-3 bg-slate-800/50 border border-slate-700 rounded-lg">
            <div class="text-xs text-slate-400">文件数量</div>
            <div class="text-sm font-medium mt-1">{{ Object.keys(preview.manifest.files).length }} 个文件</div>
          </div>
        </div>

        <!-- Services -->
        <div class="mb-4">
          <div class="text-sm font-medium text-slate-300 mb-2">服务列表</div>
          <div class="space-y-2">
            <div
              v-for="svc in preview.manifest.services"
              :key="svc.name"
              class="flex items-center justify-between p-2 bg-slate-800/30 border border-slate-700 rounded-lg text-sm"
            >
              <span class="font-mono text-blue-300">{{ svc.name }}</span>
              <span class="text-slate-400">{{ svc.image }}:{{ svc.version }}</span>
            </div>
          </div>
        </div>

        <!-- Errors in manifest -->
        <div v-if="preview.manifest.errors.length > 0" class="mb-4 p-3 bg-amber-500/10 border border-amber-500/20 rounded-lg">
          <div class="text-sm font-medium text-amber-400 mb-1">备份时的警告</div>
          <div v-for="err in preview.manifest.errors" :key="err" class="text-xs text-amber-300">{{ err }}</div>
        </div>

        <!-- Missing images -->
        <div v-if="preview.missing_images.length > 0" class="mb-4 p-3 bg-amber-500/10 border border-amber-500/20 rounded-lg">
          <div class="text-sm font-medium text-amber-400 mb-1">缺失的 Docker 镜像</div>
          <div v-for="img in preview.missing_images" :key="img" class="text-xs text-amber-300 font-mono">{{ img }}</div>
        </div>

        <!-- Port Conflicts -->
        <div v-if="preview.port_conflicts.length > 0" class="mb-4">
          <div class="flex items-center justify-between mb-2">
            <div class="text-sm font-medium text-amber-400">端口冲突</div>
            <button
              @click="applyAutoAssign(preview.port_conflicts)"
              class="text-xs px-3 py-1 bg-blue-600/20 text-blue-400 border border-blue-600/30 rounded hover:bg-blue-600 hover:text-white transition"
            >
              自动分配
            </button>
          </div>
          <div class="space-y-2">
            <div
              v-for="conflict in preview.port_conflicts"
              :key="conflict.service"
              class="flex items-center gap-3 p-2 bg-slate-800/30 border border-amber-500/20 rounded-lg text-sm"
            >
              <span class="text-slate-300">{{ conflict.service }}</span>
              <span class="text-rose-400">端口 {{ conflict.port }} 已占用</span>
              <span class="text-slate-500">→</span>
              <input
                v-model.number="portOverrides[conflict.service]"
                type="number"
                class="w-24 bg-slate-800 border border-slate-700 rounded px-2 py-1 text-xs outline-none focus:ring-1 focus:ring-blue-500"
              />
            </div>
          </div>
        </div>
      </section>

      <!-- Progress -->
      <section v-if="progress" class="bg-slate-900 border border-slate-800 rounded-xl p-6">
        <h2 class="text-lg font-bold mb-4">📊 恢复进度</h2>
        <div class="mb-2 text-sm text-slate-300">{{ progress.step }}</div>
        <div class="w-full bg-slate-800 rounded-full h-3 overflow-hidden">
          <div
            class="h-full bg-emerald-600 rounded-full transition-all duration-300"
            :style="{ width: progress.percentage + '%' }"
          ></div>
        </div>
        <div class="text-xs text-slate-500 mt-1 text-right">{{ progress.percentage }}%</div>
      </section>

      <!-- Action -->
      <section v-if="preview" class="bg-slate-900 border border-slate-800 rounded-xl p-6">
        <button
          @click="handleRestore"
          :disabled="restoring"
          class="w-full py-3 bg-emerald-600 hover:bg-emerald-700 rounded-xl font-bold transition disabled:opacity-50 flex items-center justify-center gap-2"
        >
          <span v-if="restoring" class="inline-block animate-spin rounded-full h-4 w-4 border-b-2 border-white"></span>
          {{ restoring ? '恢复中...' : '开始恢复' }}
        </button>
      </section>
    </div>
  </div>
</template>
