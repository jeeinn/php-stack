<script setup lang="ts">
import { ref, computed, onUnmounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import type { RestorePreview, RestoreProgress } from '../types/env-config';
import { showToast } from '../composables/useToast';
import { showConfirm } from '../composables/useConfirmDialog';

const { t } = useI18n();

type RestoreStep = 'select' | 'preview' | 'verify' | 'restore';

const zipPath = ref('');
const preview = ref<RestorePreview | null>(null);
const verified = ref<boolean | null>(null);
const restoring = ref(false);
const loading = ref(false);
const progress = ref<RestoreProgress | null>(null);

const currentStep = ref<RestoreStep>('select');
const completedSteps = ref<Set<RestoreStep>>(new Set());

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

function goToStep(step: RestoreStep) {
  currentStep.value = step;
}

function markStepCompleted(step: RestoreStep) {
  completedSteps.value.add(step);
}

function resetSteps() {
  currentStep.value = 'select';
  completedSteps.value.clear();
}

const canRestore = computed(() => {
  if (!preview.value) return false;
  if (verified.value !== true) return false;
  return true;
});

const isStepCompleted = (step: RestoreStep) => completedSteps.value.has(step);
const isStepActive = (step: RestoreStep) => currentStep.value === step;

const stepStatus = computed(() => {
  return {
    select: {
      completed: isStepCompleted('select'),
      active: isStepActive('select'),
      icon: zipPath.value ? '✓' : '1'
    },
    preview: {
      completed: isStepCompleted('preview'),
      active: isStepActive('preview'),
      icon: preview.value ? '✓' : '2'
    },
    verify: {
      completed: isStepCompleted('verify'),
      active: isStepActive('verify'),
      icon: verified.value === true ? '✓' : '3'
    },
    restore: {
      completed: isStepCompleted('restore'),
      active: isStepActive('restore'),
      icon: '4'
    }
  };
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
    resetSteps();
    markStepCompleted('select');
    goToStep('preview');
  }
}

async function handlePreview() {
  if (!zipPath.value) return;
  loading.value = true;
  try {
    preview.value = await invoke<RestorePreview>('preview_restore', { zipPath: zipPath.value });
    markStepCompleted('preview');
    showToast(t('restore.toast.previewDone'), 'success');
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
    if (verified.value) {
      markStepCompleted('verify');
      showToast(t('restore.toast.verifyPassed'), 'success');
    } else {
      showToast(t('restore.toast.verifyFailed'), 'error');
    }
  } catch (e) {
    showToast(e as string, 'error');
    verified.value = false;
  } finally {
    loading.value = false;
  }
}

async function handleRestore() {
  if (!zipPath.value || !canRestore.value) return;
  
  const confirmed = await showConfirm({
    title: t('restore.confirm.title'),
    message: t('restore.confirm.message'),
    confirmText: t('restore.confirm.start'),
    cancelText: t('common.cancel'),
    type: 'warning'
  });
  
  if (!confirmed) return;
  
  restoring.value = true;
  progress.value = { step: t('common.loading'), percentage: 0 };

  try {
    await invoke('execute_restore', {
      zipPath: zipPath.value,
    });
    showToast(t('restore.toast.success'), 'success');
    progress.value = { step: '✅', percentage: 100 };
    markStepCompleted('restore');
  } catch (e) {
    showToast(e as string, 'error');
  } finally {
    restoring.value = false;
  }
}

function formatTimestamp(ts: string): string {
  try {
    return new Date(ts).toLocaleString();
  } catch {
    return ts;
  }
}
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden">
    <header class="mb-6">
      <h1 class="text-3xl font-bold text-slate-900 dark:text-slate-200">{{ $t('restore.title') }}</h1>
      <p class="text-slate-500 dark:text-slate-400 text-sm mt-1">{{ $t('restore.subtitle') }}</p>
    </header>

    <div class="flex-1 overflow-y-auto pr-2 space-y-6">
      <!-- Step Indicator -->
      <section class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-6">
        <div class="flex items-center justify-between relative">
          <div class="absolute top-5 left-0 right-0 h-0.5 bg-slate-200 dark:bg-slate-700 -z-10"></div>
          
          <div class="flex flex-col items-center gap-2 bg-white dark:bg-slate-900 px-2">
            <div :class="['w-10 h-10 rounded-full flex items-center justify-center font-bold text-sm transition-all duration-300', stepStatus.select.completed ? 'bg-emerald-600 text-white' : stepStatus.select.active ? 'bg-blue-600 text-white ring-4 ring-blue-600/20' : 'bg-slate-200 dark:bg-slate-700 text-slate-400']">
              {{ stepStatus.select.icon }}
            </div>
            <span :class="['text-xs font-medium', stepStatus.select.active ? 'text-blue-600 dark:text-blue-400' : 'text-slate-500 dark:text-slate-500']">{{ $t('restore.steps.select') }}</span>
          </div>

          <div class="flex flex-col items-center gap-2 bg-white dark:bg-slate-900 px-2">
            <div :class="['w-10 h-10 rounded-full flex items-center justify-center font-bold text-sm transition-all duration-300', stepStatus.preview.completed ? 'bg-emerald-600 text-white' : stepStatus.preview.active ? 'bg-blue-600 text-white ring-4 ring-blue-600/20' : 'bg-slate-200 dark:bg-slate-700 text-slate-400']">
              {{ stepStatus.preview.icon }}
            </div>
            <span :class="['text-xs font-medium', stepStatus.preview.active ? 'text-blue-600 dark:text-blue-400' : 'text-slate-500 dark:text-slate-500']">{{ $t('restore.steps.preview') }}</span>
          </div>

          <div class="flex flex-col items-center gap-2 bg-white dark:bg-slate-900 px-2">
            <div :class="['w-10 h-10 rounded-full flex items-center justify-center font-bold text-sm transition-all duration-300', stepStatus.verify.completed ? 'bg-emerald-600 text-white' : stepStatus.verify.active ? 'bg-blue-600 text-white ring-4 ring-blue-600/20' : 'bg-slate-200 dark:bg-slate-700 text-slate-400']">
              {{ stepStatus.verify.icon }}
            </div>
            <span :class="['text-xs font-medium', stepStatus.verify.active ? 'text-blue-600 dark:text-blue-400' : 'text-slate-500 dark:text-slate-500']">{{ $t('restore.steps.verify') }}</span>
          </div>

          <div class="flex flex-col items-center gap-2 bg-white dark:bg-slate-900 px-2">
            <div :class="['w-10 h-10 rounded-full flex items-center justify-center font-bold text-sm transition-all duration-300', stepStatus.restore.completed ? 'bg-emerald-600 text-white' : stepStatus.restore.active ? 'bg-blue-600 text-white ring-4 ring-blue-600/20' : 'bg-slate-200 dark:bg-slate-700 text-slate-400']">
              {{ stepStatus.restore.icon }}
            </div>
            <span :class="['text-xs font-medium', stepStatus.restore.active ? 'text-blue-600 dark:text-blue-400' : 'text-slate-500 dark:text-slate-500']">{{ $t('restore.steps.restore') }}</span>
          </div>
        </div>
      </section>

      <div class="space-y-6">
        <!-- Step 1: File Selection -->
        <Transition name="step-fade">
          <section v-if="currentStep === 'select'" class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-6">
            <h2 class="text-lg font-bold mb-4 text-slate-900 dark:text-slate-200">{{ $t('restore.selectFile.title') }}</h2>
            <div class="flex gap-3">
              <input :value="zipPath" readonly :placeholder="$t('restore.selectFile.placeholder')" class="flex-1 bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-lg px-4 py-2 text-sm outline-none text-slate-500 dark:text-slate-400" />
              <button @click="selectFile" class="px-5 py-2 bg-blue-600 hover:bg-blue-700 border border-blue-600 rounded-lg font-medium transition text-white">
                {{ $t('restore.selectFile.browse') }}
              </button>
            </div>
            <div class="mt-4 text-xs text-center text-slate-500 dark:text-slate-500">{{ $t('restore.selectFile.hint') }}</div>
          </section>
        </Transition>

        <!-- Step 2: Preview -->
        <Transition name="step-fade">
          <section v-if="currentStep === 'preview'" class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-6">
            <div v-if="!preview">
              <h2 class="text-lg font-bold mb-4 text-slate-900 dark:text-slate-200">{{ $t('restore.preview.title') }}</h2>
              <button @click="handlePreview" :disabled="loading" class="w-full py-3 bg-blue-600 hover:bg-blue-700 rounded-xl font-bold transition disabled:opacity-50 flex items-center justify-center gap-2 text-white">
                <span v-if="loading" class="inline-block animate-spin rounded-full h-4 w-4 border-b-2 border-white"></span>
                {{ loading ? $t('restore.preview.previewing') : $t('restore.preview.startPreview') }}
              </button>
              <div class="mt-4 text-xs text-center text-slate-500 dark:text-slate-500">{{ $t('restore.preview.hint') }}</div>
            </div>

            <div v-else>
              <h2 class="text-lg font-bold mb-4 text-slate-900 dark:text-slate-200">{{ $t('restore.preview.summaryTitle') }}</h2>
              
              <div class="grid grid-cols-2 gap-4 mb-4">
                <div class="p-3 bg-slate-50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded-lg">
                  <div class="text-xs text-slate-600 dark:text-slate-400">{{ $t('restore.preview.backupTime') }}</div>
                  <div class="text-sm font-medium mt-1 text-slate-900 dark:text-slate-200">{{ formatTimestamp(preview.manifest.timestamp) }}</div>
                </div>
                <div class="p-3 bg-slate-50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded-lg">
                  <div class="text-xs text-slate-600 dark:text-slate-400">{{ $t('restore.preview.appVersion') }}</div>
                  <div class="text-sm font-medium mt-1 text-slate-900 dark:text-slate-200">{{ preview.manifest.app_version }}</div>
                </div>
                <div class="p-3 bg-slate-50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded-lg">
                  <div class="text-xs text-slate-600 dark:text-slate-400">{{ $t('restore.preview.osInfo') }}</div>
                  <div class="text-sm font-medium mt-1 text-slate-900 dark:text-slate-200">{{ preview.manifest.os_info }}</div>
                </div>
                <div class="p-3 bg-slate-50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded-lg">
                  <div class="text-xs text-slate-600 dark:text-slate-400">{{ $t('restore.preview.fileCount') }}</div>
                  <div class="text-sm font-medium mt-1 text-slate-900 dark:text-slate-200">{{ $t('restore.preview.fileCountValue', { count: Object.keys(preview.manifest.files).length }) }}</div>
                </div>
              </div>

              <div class="mb-4">
                <div class="text-sm font-medium text-slate-700 dark:text-slate-300 mb-2">{{ $t('restore.preview.serviceList') }}</div>
                <div class="space-y-2">
                  <div v-for="svc in preview.manifest.services" :key="svc.name" class="flex items-center justify-between p-2 bg-slate-50 dark:bg-slate-800/30 border border-slate-200 dark:border-slate-700 rounded-lg text-sm">
                    <span class="font-mono text-blue-600 dark:text-blue-300">{{ svc.name }}</span>
                    <span class="text-slate-600 dark:text-slate-400">{{ svc.image }}:{{ svc.version }}</span>
                  </div>
                </div>
              </div>

              <div v-if="preview.manifest.errors.length > 0" class="mb-4 p-3 bg-amber-500/10 border border-amber-500/20 rounded-lg">
                <div class="text-sm font-medium text-amber-600 dark:text-amber-400 mb-1">{{ $t('restore.preview.warnings') }}</div>
                <div v-for="err in preview.manifest.errors" :key="err" class="text-xs text-amber-600 dark:text-amber-300">{{ err }}</div>
              </div>

              <div class="flex gap-3">
                <button disabled class="flex-1 py-3 bg-emerald-600 rounded-xl font-bold flex items-center justify-center gap-2 opacity-50 cursor-not-allowed text-white">
                  {{ $t('restore.preview.done') }}
                </button>
                <button @click="goToStep('verify')" class="flex-1 py-3 bg-blue-600 hover:bg-blue-700 rounded-xl font-bold transition flex items-center justify-center gap-2 text-white">
                  {{ $t('restore.preview.next') }}
                </button>
              </div>
            </div>
          </section>
        </Transition>

        <!-- Step 3: Verify -->
        <Transition name="step-fade">
          <section v-if="currentStep === 'verify'" class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-6">
            <div v-if="verified === null">
              <h2 class="text-lg font-bold mb-4 text-slate-900 dark:text-slate-200">{{ $t('restore.verify.title') }}</h2>
              <button @click="handleVerify" :disabled="loading" class="w-full py-3 bg-emerald-600 hover:bg-emerald-700 rounded-xl font-bold transition disabled:opacity-50 flex items-center justify-center gap-2 text-white">
                <span v-if="loading" class="inline-block animate-spin rounded-full h-4 w-4 border-b-2 border-white"></span>
                {{ loading ? $t('restore.verify.verifying') : $t('restore.verify.startVerify') }}
              </button>
              <div class="mt-4 text-xs text-center text-slate-500 dark:text-slate-500">{{ $t('restore.verify.hint') }}</div>
            </div>

            <div v-else>
              <h2 class="text-lg font-bold mb-4 text-slate-900 dark:text-slate-200">{{ $t('restore.verify.resultTitle') }}</h2>
              <div v-if="verified" class="p-4 bg-emerald-500/10 border border-emerald-500/20 rounded-lg mb-4">
                <div class="flex items-center gap-2 text-emerald-600 dark:text-emerald-400">
                  <svg xmlns="http://www.w3.org/2000/svg" class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" /></svg>
                  <span class="font-medium">{{ $t('restore.verify.passed') }}</span>
                </div>
                <p class="text-xs text-emerald-600/80 dark:text-emerald-300/80 mt-2">{{ $t('restore.verify.passedHint') }}</p>
              </div>
              <div v-else class="p-4 bg-rose-500/10 border border-rose-500/20 rounded-lg mb-4">
                <div class="flex items-center gap-2 text-rose-600 dark:text-rose-400">
                  <svg xmlns="http://www.w3.org/2000/svg" class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" /></svg>
                  <span class="font-medium">{{ $t('restore.verify.failed') }}</span>
                </div>
                <p class="text-xs text-rose-600/80 dark:text-rose-300/80 mt-2">{{ $t('restore.verify.failedHint') }}</p>
              </div>
              
              <button v-if="verified" @click="goToStep('restore')" class="w-full py-3 bg-blue-600 hover:bg-blue-700 rounded-xl font-bold transition flex items-center justify-center gap-2 text-white">
                {{ $t('restore.verify.next') }}
              </button>
            </div>
          </section>
        </Transition>

        <!-- Step 4: Restore -->
        <Transition name="step-fade">
          <section v-if="currentStep === 'restore'" class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-6">
            <div v-if="!isStepCompleted('restore')">
              <h2 class="text-lg font-bold mb-4 text-slate-900 dark:text-slate-200">{{ $t('restore.restoreAction.title') }}</h2>
              <button @click="handleRestore" :disabled="!canRestore || restoring" class="w-full py-3 bg-emerald-600 hover:bg-emerald-700 rounded-xl font-bold transition disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2 text-white" :title="!canRestore ? '' : ''">
                <span v-if="restoring" class="inline-block animate-spin rounded-full h-4 w-4 border-b-2 border-white"></span>
                {{ restoring ? $t('restore.restoreAction.restoring') : $t('restore.restoreAction.start') }}
              </button>
              <div v-if="!canRestore && !restoring" class="mt-3 text-xs text-center text-slate-500 dark:text-slate-500">
                <span v-if="verified === false">{{ $t('restore.restoreAction.verifyFailed') }}</span>
              </div>
            </div>

            <div v-else class="text-center py-8">
              <div class="inline-flex items-center justify-center w-16 h-16 bg-emerald-500/20 rounded-full mb-4">
                <svg xmlns="http://www.w3.org/2000/svg" class="w-8 h-8 text-emerald-600 dark:text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" /></svg>
              </div>
              <h3 class="text-xl font-bold text-emerald-600 dark:text-emerald-400 mb-2">{{ $t('restore.success.title') }}</h3>
              <p class="text-sm text-slate-600 dark:text-slate-400">{{ $t('restore.success.description') }}</p>
            </div>
          </section>
        </Transition>

        <!-- Progress Bar -->
        <Transition name="step-fade">
          <section v-if="progress && restoring" class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-6">
            <h2 class="text-lg font-bold mb-4 text-slate-900 dark:text-slate-200">{{ $t('restore.progress.title') }}</h2>
            <div class="mb-2 text-sm text-slate-700 dark:text-slate-300">{{ progress.step }}</div>
            <div class="w-full bg-slate-200 dark:bg-slate-800 rounded-full h-3 overflow-hidden">
              <div class="h-full bg-emerald-600 rounded-full transition-all duration-300" :style="{ width: progress.percentage + '%' }"></div>
            </div>
            <div class="text-xs text-slate-500 dark:text-slate-500 mt-1 text-right">{{ progress.percentage }}%</div>
          </section>
        </Transition>
      </div>
    </div>
  </div>
</template>

<style scoped>
.step-fade-enter-active,
.step-fade-leave-active {
  transition: all 0.3s ease;
}
.step-fade-enter-from {
  opacity: 0;
  transform: translateY(10px);
}
.step-fade-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}
</style>
