<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { save } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import type { BackupOptions, BackupProgress, SiteEntry } from '../types/env-config';
import {
  createBackup,
  getBackupOptions,
  saveBackupOptions,
  loadExistingConfig,
  normalizeError,
} from '../api';
import { showToast } from '../composables/useToast';

const { t } = useI18n();

const DEFAULT_PATTERNS = [
  '.env',
  '.env.*',
  '*.local.php',
  '*.local.yml',
];

const options = ref<BackupOptions>({
  include_projects: false,
  project_patterns: [...DEFAULT_PATTERNS],
  include_logs: false,
  pack_full_tree: false,
});

const projectPatternsText = ref(DEFAULT_PATTERNS.join('\n'));
const knownSites = ref<SiteEntry[]>([]);
const selectedSiteIds = ref<string[]>([]);
const hydrated = ref(false);

function buildPersistedOptions(): BackupOptions {
  return {
    include_projects: options.value.include_projects,
    include_logs: options.value.include_logs,
    pack_full_tree: !!options.value.pack_full_tree,
    project_patterns: projectPatternsText.value
      .split('\n')
      .map((l) => l.trim())
      .filter(Boolean),
    site_ids: [...selectedSiteIds.value],
  };
}

async function persistOptions() {
  if (!hydrated.value) return;
  try {
    await saveBackupOptions(buildPersistedOptions());
  } catch (e) {
    console.error('[Backup] failed to save options:', e);
  }
}

onMounted(async () => {
  try {
    const [config, saved] = await Promise.all([
      loadExistingConfig(),
      getBackupOptions(),
    ]);
    knownSites.value = config?.sites ?? [];

    options.value = {
      include_projects: saved.include_projects,
      include_logs: saved.include_logs,
      pack_full_tree: !!saved.pack_full_tree,
      project_patterns: saved.project_patterns?.length
        ? saved.project_patterns
        : [...DEFAULT_PATTERNS],
      site_ids: saved.site_ids ?? [],
    };
    projectPatternsText.value = (options.value.project_patterns ?? DEFAULT_PATTERNS).join('\n');

    const knownIds = new Set(knownSites.value.map((s) => s.id));
    selectedSiteIds.value = (saved.site_ids ?? []).filter((id) => knownIds.has(id));
  } catch (e) {
    console.error('[Backup] failed to load options/sites:', e);
  } finally {
    hydrated.value = true;
  }
});

const backing = ref(false);
const progress = ref<BackupProgress | null>(null);

let unlisten: (() => void) | null = null;

async function setupListener() {
  unlisten = await listen<BackupProgress>('backup-progress', (event) => {
    progress.value = event.payload;
  });
}
setupListener();

onUnmounted(() => {
  void persistOptions();
  if (unlisten) unlisten();
});

async function handleBackup() {
  await persistOptions();

  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, '0');
  const day = String(now.getDate()).padStart(2, '0');
  const hours = String(now.getHours()).padStart(2, '0');
  const minutes = String(now.getMinutes()).padStart(2, '0');
  const seconds = String(now.getSeconds()).padStart(2, '0');
  const timestamp = `${year}${month}${day}-${hours}${minutes}${seconds}`;

  const savePath = await save({
    filters: [{ name: 'PHP-Stack Backup', extensions: ['zip'] }],
    defaultPath: `php-stack-backup-${timestamp}.zip`,
  });

  if (!savePath) return;

  backing.value = true;
  progress.value = { step: 'common.loading', percentage: 0 };

  try {
    const persisted = buildPersistedOptions();
    const backupOptions: BackupOptions = {
      ...persisted,
      project_patterns: options.value.include_projects && !options.value.pack_full_tree
        ? persisted.project_patterns
        : [],
      site_ids: options.value.include_projects ? [...selectedSiteIds.value] : [],
      pack_full_tree: options.value.include_projects && !!options.value.pack_full_tree,
    };

    await createBackup(savePath, backupOptions);
    showToast(t('backup.toast.success', { path: savePath }), 'success');
    progress.value = { step: 'backup.progress.steps.done', percentage: 100 };
  } catch (e) {
    showToast(normalizeError(e), 'error');
  } finally {
    backing.value = false;
  }
}
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden">
    <header class="mb-6">
      <h1 class="text-3xl font-bold text-slate-900 dark:text-slate-200">{{ $t('backup.title') }}</h1>
      <p class="text-slate-500 dark:text-slate-400 text-sm mt-1">{{ $t('backup.subtitle') }}</p>
    </header>

    <div class="flex-1 overflow-y-auto pr-2 space-y-6">
      <!-- Backup Options -->
      <section class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-6">
        <h2 class="text-lg font-bold mb-4 text-slate-900 dark:text-slate-200">{{ $t('backup.options.title') }}</h2>
        <div class="space-y-4">
          <label class="flex items-center gap-3 text-sm text-slate-700 dark:text-slate-400">
            <input type="checkbox" checked disabled class="accent-blue-500" />
            <span>{{ $t('backup.options.coreConfig') }}</span>
          </label>

          <div>
            <label class="flex items-center gap-3 text-sm text-slate-700 dark:text-slate-300 cursor-pointer">
              <input type="checkbox" v-model="options.include_projects" class="accent-blue-500" />
              <span>{{ $t('backup.options.includeProjects') }}</span>
            </label>
            <transition name="fade">
              <div v-if="options.include_projects" class="mt-3 ml-7 space-y-3">
                <div v-if="knownSites.length > 0" class="space-y-2">
                  <div class="text-xs text-slate-600 dark:text-slate-400">{{ $t('backup.options.sites') }}</div>
                  <label
                    v-for="site in knownSites"
                    :key="site.id"
                    class="flex items-center gap-2 text-xs text-slate-700 dark:text-slate-300"
                  >
                    <input type="checkbox" :value="site.id" v-model="selectedSiteIds" class="accent-blue-500" />
                    <span>{{ $t('backup.options.siteItem', { name: site.server_name || site.id, path: site.public_dir ? `${site.host_path} / ${site.public_dir}` : site.host_path }) }}</span>
                  </label>
                  <p class="text-[10px] text-slate-500">{{ $t('backup.options.outsideHint') }}</p>
                </div>
                <p v-else class="text-xs text-slate-500">{{ $t('backup.options.noSites') }}</p>

                <label class="flex items-center gap-3 text-sm text-slate-700 dark:text-slate-300 cursor-pointer">
                  <input type="checkbox" v-model="options.pack_full_tree" class="accent-blue-500" />
                  <div class="flex flex-col">
                    <span>{{ $t('backup.options.packFullTree') }}</span>
                    <span class="text-xs text-slate-500">{{ $t('backup.options.packFullTreeHint') }}</span>
                  </div>
                </label>

                <div v-if="!options.pack_full_tree">
                  <label class="block text-xs text-slate-600 dark:text-slate-400 mb-1">{{ $t('backup.options.patterns') }}</label>
                  <textarea
                    v-model="projectPatternsText"
                    :placeholder="$t('backup.options.patternsPlaceholder')"
                    class="w-full h-24 bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-lg p-3 text-xs font-mono text-slate-900 dark:text-blue-300 focus:ring-1 focus:ring-blue-500 outline-none"
                  ></textarea>
                  <p class="text-[10px] text-slate-500 dark:text-slate-500 mt-1">
                    {{ $t('backup.options.patternsHint') }}
                  </p>
                </div>
              </div>
            </transition>
          </div>

          <label class="flex items-center gap-3 text-sm text-slate-700 dark:text-slate-300 cursor-pointer">
            <input type="checkbox" v-model="options.include_logs" class="accent-blue-500" />
            <div class="flex flex-col">
              <span>{{ $t('backup.options.includeLogs') }}</span>
              <span class="text-xs text-slate-500 dark:text-slate-500">{{ $t('backup.options.logsWarning') }}</span>
            </div>
          </label>
        </div>
      </section>

      <!-- Progress -->
      <section v-if="progress" class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-6">
        <h2 class="text-lg font-bold mb-4 text-slate-900 dark:text-slate-200">{{ $t('backup.progress.title') }}</h2>
        <div class="mb-2 text-sm text-slate-700 dark:text-slate-300">{{ $t(progress.step) }}</div>
        <div class="w-full bg-slate-200 dark:bg-slate-800 rounded-full h-3 overflow-hidden">
          <div
            class="h-full bg-blue-600 rounded-full transition-all duration-300"
            :style="{ width: progress.percentage + '%' }"
          ></div>
        </div>
        <div class="text-xs text-slate-500 dark:text-slate-500 mt-1 text-right">{{ progress.percentage }}%</div>
      </section>

      <!-- Action -->
      <section class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-6">
        <button
          @click="handleBackup"
          :disabled="backing"
          class="w-full py-3 bg-blue-600 hover:bg-blue-700 rounded-xl font-bold transition disabled:opacity-50 flex items-center justify-center gap-2 text-white"
        >
          <span v-if="backing" class="inline-block animate-spin rounded-full h-4 w-4 border-b-2 border-white"></span>
          {{ backing ? $t('backup.action.creating') : $t('backup.action.create') }}
        </button>
      </section>
    </div>
  </div>
</template>
