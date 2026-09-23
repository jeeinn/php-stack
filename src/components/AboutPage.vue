<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { save } from '@tauri-apps/plugin-dialog';
import { open } from '@tauri-apps/plugin-shell';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { exportLogsTo, getSupportInfo, type SupportInfo } from '../api';
import {
  getMinLogLevel,
  setMinLogLevel,
  showToast,
  type LogLevel,
} from '../composables/useToast';
import { pendingUpdateVersion, setPendingUpdateVersion } from '../composables/useUpdater';

const { t } = useI18n();

const PROJECT_URL = 'https://github.com/jeeinn/php-stack';

const support = ref<SupportInfo | null>(null);
const supportError = ref('');
const minLogLevel = getMinLogLevel();
const logLevels: LogLevel[] = ['info', 'warn', 'error'];

const updateStatus = ref<'idle' | 'checking' | 'upToDate' | 'available' | 'downloading' | 'error'>('idle');
const updateError = ref('');
const foundUpdate = ref<Update | null>(null);
const downloadPercent = ref(0);

const supportText = computed(() => {
  if (!support.value) return '';
  return [
    `Version: ${support.value.app_version}`,
    `OS: ${support.value.os} ${support.value.os_version}`,
    `Arch: ${support.value.arch}`,
  ].join('\n');
});

onMounted(async () => {
  try {
    support.value = await getSupportInfo();
  } catch (e) {
    supportError.value = String(e);
  }
});

async function copySupport() {
  if (!supportText.value) return;
  try {
    await writeText(supportText.value);
    showToast(t('about.copied'), 'success');
  } catch (e) {
    showToast(t('about.copyFailed', { error: e }), 'error');
  }
}

function changeLogLevel(level: LogLevel) {
  setMinLogLevel(level);
}

async function exportLogs() {
  try {
    const dest = await save({
      defaultPath: 'php-stack.log',
      filters: [{ name: 'Log', extensions: ['log', 'txt'] }],
    });
    if (!dest) return;
    await exportLogsTo(dest);
    showToast(t('dashboard.log.exported', { path: dest }), 'success');
  } catch (e) {
    showToast(t('dashboard.log.exportFailed', { error: e }), 'error');
  }
}

async function openProject() {
  await open(PROJECT_URL);
}

async function checkForUpdate() {
  updateStatus.value = 'checking';
  updateError.value = '';
  foundUpdate.value = null;
  try {
    const update = await check();
    if (!update) {
      setPendingUpdateVersion('');
      updateStatus.value = 'upToDate';
      return;
    }
    foundUpdate.value = update;
    setPendingUpdateVersion(update.version);
    updateStatus.value = 'available';
  } catch (e) {
    updateError.value = String(e);
    updateStatus.value = 'error';
  }
}

async function installUpdate() {
  const update = foundUpdate.value;
  if (!update) return;
  updateStatus.value = 'downloading';
  downloadPercent.value = 0;
  let downloaded = 0;
  let contentLength = 0;
  try {
    await update.downloadAndInstall((event) => {
      switch (event.event) {
        case 'Started':
          contentLength = event.data.contentLength ?? 0;
          downloaded = 0;
          downloadPercent.value = 0;
          break;
        case 'Progress':
          downloaded += event.data.chunkLength;
          downloadPercent.value =
            contentLength > 0 ? Math.min(100, Math.round((downloaded / contentLength) * 100)) : 0;
          break;
        case 'Finished':
          downloadPercent.value = 100;
          break;
      }
    });
    await relaunch();
  } catch (e) {
    updateError.value = String(e);
    updateStatus.value = 'error';
  }
}

const updatePercentLabel = computed(() =>
  t('about.update.downloading', { percent: downloadPercent.value }),
);
</script>

<template>
  <div class="flex-1 overflow-y-auto scrollbar-hide bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-200">
    <div class="max-w-2xl mx-auto p-6 flex flex-col gap-6">
      <h1 class="text-xl font-bold text-slate-900 dark:text-slate-100">{{ $t('about.title') }}</h1>

      <section class="p-4 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl">
        <div class="flex items-center justify-between gap-3 mb-3">
          <h2 class="text-sm font-semibold text-slate-800 dark:text-slate-200">{{ $t('about.support') }}</h2>
          <button
            type="button"
            class="px-3 py-1.5 text-xs bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg"
            data-testid="about-copy-support"
            :disabled="!support"
            @click="copySupport"
          >
            {{ $t('about.copyAll') }}
          </button>
        </div>
        <p v-if="supportError" class="text-sm text-rose-500">{{ supportError }}</p>
        <dl v-else-if="support" class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
          <dt class="text-slate-500">{{ $t('about.version') }}</dt>
          <dd class="font-mono" data-testid="about-version">{{ support.app_version }}</dd>
          <dt class="text-slate-500">{{ $t('about.os') }}</dt>
          <dd class="font-mono" data-testid="about-os">{{ support.os }} {{ support.os_version }}</dd>
          <dt class="text-slate-500">{{ $t('about.arch') }}</dt>
          <dd class="font-mono" data-testid="about-arch">{{ support.arch }}</dd>
        </dl>
        <p v-else class="text-sm text-slate-500">{{ $t('common.loading') }}</p>
      </section>

      <section class="p-4 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl">
        <h2 class="text-sm font-semibold text-slate-800 dark:text-slate-200 mb-1">{{ $t('about.logLevel') }}</h2>
        <p class="text-xs text-slate-500 mb-3">{{ $t('about.logLevelHint') }}</p>
        <div class="flex gap-2" data-testid="about-log-level">
          <button
            v-for="level in logLevels"
            :key="level"
            type="button"
            class="px-3 py-1.5 text-xs rounded-lg font-medium transition"
            :class="minLogLevel === level
              ? 'bg-blue-600 text-white'
              : 'bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'"
            @click="changeLogLevel(level)"
          >
            {{ $t(`about.level.${level}`) }}
          </button>
        </div>
      </section>

      <section class="p-4 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
        <div>
          <h2 class="text-sm font-semibold text-slate-800 dark:text-slate-200">{{ $t('about.exportLogs') }}</h2>
        </div>
        <button
          type="button"
          class="px-4 py-2 text-sm bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg font-medium"
          data-testid="about-export-logs"
          @click="exportLogs"
        >
          {{ $t('about.exportLogs') }}
        </button>
      </section>

      <section class="p-4 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
        <div>
          <h2 class="text-sm font-semibold text-slate-800 dark:text-slate-200">{{ $t('about.project') }}</h2>
          <p class="text-xs font-mono text-slate-500 mt-1 break-all">{{ PROJECT_URL }}</p>
        </div>
        <button
          type="button"
          class="px-4 py-2 text-sm bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg font-medium"
          data-testid="about-open-repo"
          @click="openProject"
        >
          {{ $t('about.openRepo') }}
        </button>
      </section>

      <section class="p-4 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl">
        <h2 class="text-sm font-semibold text-slate-800 dark:text-slate-200 mb-3">{{ $t('about.update.title') }}</h2>
        <div class="flex flex-wrap items-center gap-2">
          <button
            type="button"
            class="px-4 py-2 text-sm ui-btn-primary rounded-lg font-medium disabled:opacity-60"
            data-testid="about-check-update"
            :disabled="updateStatus === 'checking' || updateStatus === 'downloading'"
            @click="checkForUpdate"
          >
            {{ updateStatus === 'checking' ? $t('about.update.checking') : $t('about.update.check') }}
          </button>
          <button
            v-if="updateStatus === 'available'"
            type="button"
            class="px-4 py-2 text-sm ui-btn-primary rounded-lg font-medium"
            data-testid="about-install-update"
            @click="installUpdate"
          >
            {{ $t('about.update.download') }}
          </button>
        </div>
        <p v-if="updateStatus === 'upToDate'" class="mt-3 text-sm text-emerald-600 dark:text-emerald-400" data-testid="about-update-uptodate">
          {{ $t('about.update.upToDate') }}
        </p>
        <div v-else-if="updateStatus === 'available' && foundUpdate" class="mt-3 text-sm" data-testid="about-update-available">
          <p class="text-blue-600 dark:text-blue-400 font-medium">
            {{ $t('about.update.available', { version: foundUpdate.version }) }}
          </p>
          <p v-if="foundUpdate.body" class="mt-2 text-xs text-slate-500 whitespace-pre-wrap">{{ foundUpdate.body }}</p>
        </div>
        <p v-else-if="updateStatus === 'downloading'" class="mt-3 text-sm text-slate-600 dark:text-slate-300">
          {{ updatePercentLabel }}
        </p>
        <p v-else-if="updateStatus === 'error'" class="mt-3 text-sm text-rose-500" data-testid="about-update-error">
          {{ $t('about.update.failed', { error: updateError }) }}
        </p>
        <p v-if="pendingUpdateVersion && updateStatus !== 'available'" class="mt-2 text-xs text-blue-600 dark:text-blue-400">
          {{ $t('about.update.available', { version: pendingUpdateVersion }) }}
        </p>
        <p class="mt-2 text-xs text-slate-500">{{ $t('about.update.devHint') }}</p>
      </section>
    </div>
  </div>
</template>

<style scoped>
@reference "tailwindcss";
</style>
