<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

const { t } = useI18n();

const isOpen = ref(false);
const currentPath = ref('');
const errorMessage = ref('');

onMounted(async () => {
  try {
    const info = await invoke<any>('get_workspace_info');
    if (!info) {
      isOpen.value = true;
    } else {
      currentPath.value = info.workspace_path;
    }
  } catch (e) {
    console.error('Failed to load workspace info:', e);
    isOpen.value = true;
  }
});

async function selectDirectory() {
  const selected = await open({
    directory: true,
    multiple: false,
  });
  if (selected) {
    currentPath.value = selected as string;
    errorMessage.value = '';
  }
}

async function confirmWorkspace() {
  if (!currentPath.value) {
    errorMessage.value = t('workspace.error.invalidPath');
    return;
  }

  try {
    await invoke('set_workspace_path', { path: currentPath.value });
    isOpen.value = false;
    window.location.reload();
  } catch (e) {
    errorMessage.value = e as string;
  }
}
</script>

<template>
  <div v-if="isOpen" class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50">
    <div class="bg-slate-900 border border-slate-700 rounded-xl p-8 max-w-md w-full shadow-2xl">
      <h2 class="text-2xl font-bold text-white mb-4">{{ $t('workspace.title') }}</h2>
      <p class="text-slate-400 mb-6">
        {{ $t('workspace.description', { bold: '' }) }}<strong>{{ $t('workspace.bold') }}</strong>
      </p>

      <div class="space-y-4">
        <div>
          <label class="block text-sm font-medium text-slate-300 mb-2">{{ $t('workspace.pathLabel') }}</label>
          <div class="flex gap-2">
            <input 
              v-model="currentPath" 
              readonly 
              :placeholder="$t('workspace.pathPlaceholder')"
              class="flex-1 bg-slate-800 border border-slate-700 rounded-lg px-4 py-2 text-sm text-slate-200 outline-none"
            />
            <button 
              @click="selectDirectory"
              class="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition"
            >
              {{ $t('common.browse') }}
            </button>
          </div>
        </div>

        <div v-if="errorMessage" class="text-rose-400 text-sm bg-rose-500/10 p-3 rounded-lg border border-rose-500/20">
          {{ errorMessage }}
        </div>

        <div class="pt-4 flex justify-end">
          <button 
            @click="confirmWorkspace"
            class="px-6 py-2 bg-emerald-600 hover:bg-emerald-700 text-white rounded-lg font-bold transition shadow-lg shadow-emerald-600/20"
          >
            {{ $t('workspace.confirmStart') }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
@reference "tailwindcss";
</style>
