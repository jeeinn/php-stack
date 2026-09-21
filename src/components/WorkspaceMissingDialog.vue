<script setup lang="ts">
import { ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

export interface WorkspaceMissingInfo {
  workspace_path: string;
  effective_path: string;
}

const props = defineProps<{
  open: boolean;
  info: WorkspaceMissingInfo | null;
}>();

const emit = defineEmits<{
  resolved: [];
  /** 用户选择本会话临时使用回退目录 */
  dismissTemp: [];
}>();

const { t } = useI18n();
const busy = ref(false);
const errorMessage = ref('');

watch(
  () => props.open,
  (v) => {
    if (v) errorMessage.value = '';
  },
);

async function recreate() {
  busy.value = true;
  errorMessage.value = '';
  try {
    await invoke('recreate_workspace_dir');
    emit('resolved');
  } catch (e) {
    errorMessage.value = String(e);
  } finally {
    busy.value = false;
  }
}

async function chooseNew() {
  busy.value = true;
  errorMessage.value = '';
  try {
    const selected = await open({ directory: true, multiple: false });
    if (!selected) {
      busy.value = false;
      return;
    }
    await invoke('set_workspace_path', { path: selected as string });
    emit('resolved');
  } catch (e) {
    errorMessage.value = String(e);
  } finally {
    busy.value = false;
  }
}

function useTempFallback() {
  emit('dismissTemp');
}
</script>

<template>
  <div
    v-if="open && info"
    class="fixed inset-0 z-[90] flex items-center justify-center bg-black/60 backdrop-blur-sm p-4"
    data-testid="workspace-missing-dialog"
  >
    <div
      class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-700 rounded-xl p-6 sm:p-8 max-w-lg w-full shadow-2xl"
    >
      <h2 class="text-xl font-bold text-slate-900 dark:text-slate-100 mb-2">
        {{ $t('workspace.missing.title') }}
      </h2>
      <p class="text-sm text-slate-600 dark:text-slate-400 mb-4 whitespace-pre-line">
        {{
          $t('workspace.missing.description', {
            configured: info.workspace_path,
            effective: info.effective_path,
          })
        }}
      </p>

      <div
        v-if="errorMessage"
        class="mb-4 text-sm text-rose-600 dark:text-rose-400 bg-rose-500/10 p-3 rounded-lg border border-rose-500/20"
      >
        {{ errorMessage }}
      </div>

      <div class="flex flex-col gap-2">
        <button
          type="button"
          data-testid="workspace-missing-recreate"
          :disabled="busy"
          @click="recreate"
          class="w-full px-4 py-2.5 bg-emerald-600 hover:bg-emerald-700 disabled:opacity-50 text-white rounded-lg font-medium transition"
        >
          {{ $t('workspace.missing.recreate') }}
        </button>
        <button
          type="button"
          data-testid="workspace-missing-choose"
          :disabled="busy"
          @click="chooseNew"
          class="w-full px-4 py-2.5 bg-blue-600 hover:bg-blue-700 disabled:opacity-50 text-white rounded-lg font-medium transition"
        >
          {{ $t('workspace.missing.chooseNew') }}
        </button>
        <button
          type="button"
          data-testid="workspace-missing-temp"
          :disabled="busy"
          @click="useTempFallback"
          class="w-full px-4 py-2.5 bg-white dark:bg-slate-800 hover:bg-slate-100 dark:hover:bg-slate-700 border border-slate-300 dark:border-slate-600 disabled:opacity-50 text-slate-700 dark:text-slate-300 rounded-lg font-medium transition"
        >
          {{ $t('workspace.missing.tempFallback') }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
@reference "tailwindcss";
</style>
