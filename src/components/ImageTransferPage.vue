<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { save, open } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import type {
  ImageTransferProgress,
  WorkspaceImageEntry,
} from '../types/env-config';
import {
  listWorkspaceImages,
  exportWorkspaceImages,
  importWorkspaceImages,
  normalizeError,
} from '../api';
import { showToast } from '../composables/useToast';

const { t } = useI18n();

const images = ref<WorkspaceImageEntry[]>([]);
const selected = ref<Set<string>>(new Set());
const loading = ref(false);
const exporting = ref(false);
const importing = ref(false);
const progress = ref<ImageTransferProgress | null>(null);
const importResult = ref<string[] | null>(null);

let unlisten: (() => void) | null = null;

const presentSelected = computed(() =>
  images.value.filter((i) => selected.value.has(i.ref_name) && i.present),
);

const canExport = computed(
  () => presentSelected.value.length > 0 && !exporting.value && !importing.value,
);

function roleLabel(role: string): string {
  return t(`images.role.${role}`);
}

function noteLabel(note: string | null | undefined): string {
  if (!note) return '';
  const key = `images.notes.${note}`;
  const translated = t(key);
  return translated === key ? note : translated;
}

function toggle(refName: string, present: boolean) {
  if (!present) return;
  const next = new Set(selected.value);
  if (next.has(refName)) next.delete(refName);
  else next.add(refName);
  selected.value = next;
}

function selectAllPresent() {
  selected.value = new Set(
    images.value.filter((i) => i.present).map((i) => i.ref_name),
  );
}

async function refresh() {
  loading.value = true;
  importResult.value = null;
  try {
    const list = await listWorkspaceImages();
    images.value = list;
    selected.value = new Set(list.filter((i) => i.present).map((i) => i.ref_name));
  } catch (e) {
    showToast(normalizeError(e), 'error');
    images.value = [];
    selected.value = new Set();
  } finally {
    loading.value = false;
  }
}

async function handleExport() {
  if (!canExport.value) return;

  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, '0');
  const timestamp = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}-${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;

  const savePath = await save({
    filters: [{ name: 'Docker Images', extensions: ['tar'] }],
    defaultPath: `php-stack-images-${timestamp}.tar`,
  });
  if (!savePath) return;

  exporting.value = true;
  progress.value = { step: 'images.progress.collect', percentage: 0 };
  try {
    await exportWorkspaceImages(savePath, [...selected.value]);
    showToast(t('images.toast.exportSuccess', { path: savePath }), 'success');
    progress.value = { step: 'images.progress.done', percentage: 100 };
  } catch (e) {
    showToast(normalizeError(e), 'error');
  } finally {
    exporting.value = false;
  }
}

async function handleImport() {
  const selectedPath = await open({
    multiple: false,
    filters: [{ name: 'Docker Images', extensions: ['tar'] }],
  });
  if (!selectedPath || Array.isArray(selectedPath)) return;

  importing.value = true;
  progress.value = { step: 'images.progress.load', percentage: 0 };
  importResult.value = null;
  try {
    const result = await importWorkspaceImages(selectedPath);
    importResult.value = result.loaded_refs;
    showToast(t('images.toast.importSuccess', { count: result.loaded_refs.length }), 'success');
    progress.value = { step: 'images.progress.done', percentage: 100 };
    await refresh();
  } catch (e) {
    showToast(normalizeError(e), 'error');
  } finally {
    importing.value = false;
  }
}

onMounted(async () => {
  unlisten = await listen<ImageTransferProgress>('image-transfer-progress', (event) => {
    progress.value = event.payload;
  });
  await refresh();
});

onUnmounted(() => {
  if (unlisten) unlisten();
});
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden">
    <header class="mb-6">
      <h1 class="text-3xl font-bold text-slate-900 dark:text-slate-200">{{ $t('images.title') }}</h1>
      <p class="text-slate-500 dark:text-slate-400 text-sm mt-1">{{ $t('images.subtitle') }}</p>
      <p class="text-amber-700 dark:text-amber-400/90 text-xs mt-2">{{ $t('images.hint') }}</p>
    </header>

    <div class="flex-1 overflow-y-auto pr-2 space-y-6">
      <section class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-6">
        <div class="flex flex-wrap items-center justify-between gap-3 mb-4">
          <h2 class="text-lg font-bold text-slate-900 dark:text-slate-200">{{ $t('images.list.title') }}</h2>
          <div class="flex gap-2">
            <button
              type="button"
              class="px-3 py-1.5 text-xs rounded-lg border border-slate-300 dark:border-slate-700 text-slate-700 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800"
              :disabled="loading || exporting || importing"
              @click="selectAllPresent"
            >
              {{ $t('images.list.selectAll') }}
            </button>
            <button
              type="button"
              class="px-3 py-1.5 text-xs rounded-lg border border-slate-300 dark:border-slate-700 text-slate-700 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800 disabled:opacity-50"
              :disabled="loading || exporting || importing"
              @click="refresh"
            >
              {{ loading ? $t('images.list.refreshing') : $t('images.list.refresh') }}
            </button>
          </div>
        </div>

        <div v-if="loading && images.length === 0" class="text-sm text-slate-500 py-8 text-center">
          {{ $t('common.loading') }}
        </div>
        <div v-else-if="images.length === 0" class="text-sm text-slate-500 py-8 text-center">
          {{ $t('images.list.empty') }}
        </div>
        <div v-else class="overflow-x-auto">
          <table class="w-full text-sm text-left">
            <thead class="text-xs text-slate-500 dark:text-slate-400 border-b border-slate-200 dark:border-slate-800">
              <tr>
                <th class="py-2 pr-2 w-8"></th>
                <th class="py-2 pr-3">{{ $t('images.list.ref') }}</th>
                <th class="py-2 pr-3">{{ $t('images.list.role') }}</th>
                <th class="py-2 pr-3">{{ $t('images.list.service') }}</th>
                <th class="py-2 pr-3">{{ $t('images.list.size') }}</th>
                <th class="py-2">{{ $t('images.list.status') }}</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="img in images"
                :key="img.ref_name"
                class="border-b border-slate-100 dark:border-slate-800/80"
                :class="img.present ? '' : 'opacity-60'"
              >
                <td class="py-2.5 pr-2 align-top">
                  <input
                    type="checkbox"
                    class="accent-blue-500"
                    :disabled="!img.present || exporting || importing"
                    :checked="selected.has(img.ref_name)"
                    @change="toggle(img.ref_name, img.present)"
                  />
                </td>
                <td class="py-2.5 pr-3 align-top font-mono text-xs text-slate-800 dark:text-slate-200 break-all">
                  {{ img.ref_name }}
                  <div v-if="img.source_ref" class="text-[10px] text-slate-500 mt-0.5">
                    {{ $t('images.list.fromSource', { source: img.source_ref }) }}
                  </div>
                </td>
                <td class="py-2.5 pr-3 align-top text-slate-600 dark:text-slate-400 whitespace-nowrap">
                  {{ roleLabel(img.role) }}
                </td>
                <td class="py-2.5 pr-3 align-top text-slate-600 dark:text-slate-400 whitespace-nowrap">
                  {{ img.service_dir }}
                </td>
                <td class="py-2.5 pr-3 align-top text-slate-600 dark:text-slate-400 whitespace-nowrap">
                  {{ img.size || '—' }}
                </td>
                <td class="py-2.5 align-top">
                  <span
                    class="inline-flex px-2 py-0.5 rounded text-[10px] font-medium"
                    :class="img.present
                      ? 'bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-300'
                      : 'bg-slate-100 text-slate-600 dark:bg-slate-800 dark:text-slate-400'"
                  >
                    {{ img.present ? $t('images.list.present') : $t('images.list.missing') }}
                  </span>
                  <div v-if="img.note" class="text-[10px] text-amber-700 dark:text-amber-400/90 mt-1">
                    {{ noteLabel(img.note) }}
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      <section v-if="progress" class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-6">
        <h2 class="text-lg font-bold mb-4 text-slate-900 dark:text-slate-200">{{ $t('images.progress.title') }}</h2>
        <div class="mb-2 text-sm text-slate-700 dark:text-slate-300">{{ $t(progress.step) }}</div>
        <div class="w-full bg-slate-200 dark:bg-slate-800 rounded-full h-3 overflow-hidden">
          <div
            class="h-full bg-blue-600 rounded-full transition-all duration-300"
            :style="{ width: progress.percentage + '%' }"
          ></div>
        </div>
        <div class="text-xs text-slate-500 mt-1 text-right">{{ progress.percentage }}%</div>
        <p v-if="exporting || importing" class="text-xs text-amber-700 dark:text-amber-400/90 mt-3">
          {{ $t('images.progress.doNotClose') }}
        </p>
      </section>

      <section v-if="importResult" class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-6">
        <h2 class="text-lg font-bold mb-2 text-slate-900 dark:text-slate-200">{{ $t('images.importResult.title') }}</h2>
        <p class="text-xs text-slate-500 mb-3">{{ $t('images.importResult.hint') }}</p>
        <ul class="text-xs font-mono text-slate-700 dark:text-slate-300 space-y-1 max-h-40 overflow-y-auto">
          <li v-for="refName in importResult" :key="refName">{{ refName }}</li>
        </ul>
      </section>

      <section class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-6 space-y-3">
        <button
          type="button"
          :disabled="!canExport"
          class="w-full py-3 bg-blue-600 hover:bg-blue-700 rounded-xl font-bold transition disabled:opacity-50 flex items-center justify-center gap-2 text-white"
          @click="handleExport"
        >
          <span v-if="exporting" class="inline-block animate-spin rounded-full h-4 w-4 border-b-2 border-white"></span>
          {{ exporting ? $t('images.action.exporting') : $t('images.action.export') }}
        </button>
        <button
          type="button"
          :disabled="exporting || importing"
          class="w-full py-3 bg-slate-700 hover:bg-slate-800 dark:bg-slate-700 dark:hover:bg-slate-600 rounded-xl font-bold transition disabled:opacity-50 flex items-center justify-center gap-2 text-white"
          @click="handleImport"
        >
          <span v-if="importing" class="inline-block animate-spin rounded-full h-4 w-4 border-b-2 border-white"></span>
          {{ importing ? $t('images.action.importing') : $t('images.action.import') }}
        </button>
      </section>
    </div>
  </div>
</template>
