<script setup lang="ts">
import { ref, computed, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import type { RestorePreview, RestoreProgress } from '../types/env-config';
import { showToast } from '../composables/useToast';
import { showConfirm } from '../composables/useConfirmDialog';

// Step definitions
type RestoreStep = 'select' | 'preview' | 'verify' | 'restore';

const zipPath = ref('');
const preview = ref<RestorePreview | null>(null);
const verified = ref<boolean | null>(null);
const restoring = ref(false);
const loading = ref(false);
const progress = ref<RestoreProgress | null>(null);

// Current step tracking
const currentStep = ref<RestoreStep>('select');
const completedSteps = ref<Set<RestoreStep>>(new Set());

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

// Step management functions
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

// Computed: Check if restore button should be enabled
const canRestore = computed(() => {
  // Must have preview data
  if (!preview.value) return false;
  
  // Must pass integrity verification
  if (verified.value !== true) return false;
  
  return true;
});

// Step status helpers
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

// Build simplified restore confirmation message
function buildRestoreImpactDescription(): string {
  return '⚠️ 注意: 现有配置文件将被覆盖，建议先备份当前环境\n\n💡 恢复完成后，点击"一键启动"即可运行环境';
}

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
    // Mark select step as completed and advance to preview
    markStepCompleted('select');
    goToStep('preview');
  }
}

async function handlePreview() {
  if (!zipPath.value) return;
  loading.value = true;
  try {
    preview.value = await invoke<RestorePreview>('preview_restore', { zipPath: zipPath.value });
    // Mark preview as completed but stay on this step to show results
    markStepCompleted('preview');
    showToast('预览完成，请查看备份内容', 'success');
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
      // Mark verify as completed but stay on this step to show result
      markStepCompleted('verify');
      showToast('校验通过，可以开始恢复', 'success');
    } else {
      showToast('校验失败，备份文件可能已损坏', 'error');
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
  
  // Show confirmation dialog
  const confirmed = await showConfirm({
    title: '环境恢复确认',
    message: buildRestoreImpactDescription(),
    confirmText: '开始恢复',
    cancelText: '取消',
    type: 'warning'
  });
  
  if (!confirmed) return;
  
  restoring.value = true;
  progress.value = { step: '准备中...', percentage: 0 };

  try {
    await invoke('execute_restore', {
      zipPath: zipPath.value,
    });
    showToast('环境恢复成功！', 'success');
    progress.value = { step: '完成', percentage: 100 };
    markStepCompleted('restore');
  } catch (e) {
    showToast(e as string, 'error');
  } finally {
    restoring.value = false;
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
      <!-- Step Indicator -->
      <section class="bg-slate-900 border border-slate-800 rounded-xl p-6">
        <div class="flex items-center justify-between relative">
          <!-- Connecting Line -->
          <div class="absolute top-5 left-0 right-0 h-0.5 bg-slate-700 -z-10"></div>
          
          <!-- Step 1: Select File -->
          <div class="flex flex-col items-center gap-2 bg-slate-900 px-2">
            <div 
              :class="[
                'w-10 h-10 rounded-full flex items-center justify-center font-bold text-sm transition-all duration-300',
                stepStatus.select.completed ? 'bg-emerald-600 text-white' : 
                stepStatus.select.active ? 'bg-blue-600 text-white ring-4 ring-blue-600/20' : 
                'bg-slate-700 text-slate-400'
              ]"
            >
              {{ stepStatus.select.icon }}
            </div>
            <span :class="['text-xs font-medium', stepStatus.select.active ? 'text-blue-400' : 'text-slate-500']">
              选择文件
            </span>
          </div>

          <!-- Step 2: Preview -->
          <div class="flex flex-col items-center gap-2 bg-slate-900 px-2">
            <div 
              :class="[
                'w-10 h-10 rounded-full flex items-center justify-center font-bold text-sm transition-all duration-300',
                stepStatus.preview.completed ? 'bg-emerald-600 text-white' : 
                stepStatus.preview.active ? 'bg-blue-600 text-white ring-4 ring-blue-600/20' : 
                'bg-slate-700 text-slate-400'
              ]"
            >
              {{ stepStatus.preview.icon }}
            </div>
            <span :class="['text-xs font-medium', stepStatus.preview.active ? 'text-blue-400' : 'text-slate-500']">
              预览内容
            </span>
          </div>

          <!-- Step 3: Verify -->
          <div class="flex flex-col items-center gap-2 bg-slate-900 px-2">
            <div 
              :class="[
                'w-10 h-10 rounded-full flex items-center justify-center font-bold text-sm transition-all duration-300',
                stepStatus.verify.completed ? 'bg-emerald-600 text-white' : 
                stepStatus.verify.active ? 'bg-blue-600 text-white ring-4 ring-blue-600/20' : 
                'bg-slate-700 text-slate-400'
              ]"
            >
              {{ stepStatus.verify.icon }}
            </div>
            <span :class="['text-xs font-medium', stepStatus.verify.active ? 'text-blue-400' : 'text-slate-500']">
              校验完整性
            </span>
          </div>

          <!-- Step 4: Restore -->
          <div class="flex flex-col items-center gap-2 bg-slate-900 px-2">
            <div 
              :class="[
                'w-10 h-10 rounded-full flex items-center justify-center font-bold text-sm transition-all duration-300',
                stepStatus.restore.completed ? 'bg-emerald-600 text-white' : 
                stepStatus.restore.active ? 'bg-blue-600 text-white ring-4 ring-blue-600/20' : 
                'bg-slate-700 text-slate-400'
              ]"
            >
              {{ stepStatus.restore.icon }}
            </div>
            <span :class="['text-xs font-medium', stepStatus.restore.active ? 'text-blue-400' : 'text-slate-500']">
              开始恢复
            </span>
          </div>
        </div>
      </section>

      <!-- Step Content Cards -->
      <div class="space-y-6">
        <!-- Step 1: File Selection Card -->
        <Transition name="step-fade">
          <section v-if="currentStep === 'select'" class="bg-slate-900 border border-slate-800 rounded-xl p-6">
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
                class="px-5 py-2 bg-blue-600 hover:bg-blue-700 border border-blue-600 rounded-lg font-medium transition text-white"
              >
                浏览文件
              </button>
            </div>
            <div class="mt-4 text-xs text-center text-slate-500">
              💡 支持 PHP-Stack 生成的 .zip 格式备份文件
            </div>
          </section>
        </Transition>

        <!-- Step 2: Preview Card -->
        <Transition name="step-fade">
          <section v-if="currentStep === 'preview'" class="bg-slate-900 border border-slate-800 rounded-xl p-6">
            <div v-if="!preview">
              <h2 class="text-lg font-bold mb-4">📋 预览备份内容</h2>
              <button
                @click="handlePreview"
                :disabled="loading"
                class="w-full py-3 bg-blue-600 hover:bg-blue-700 rounded-xl font-bold transition disabled:opacity-50 flex items-center justify-center gap-2"
              >
                <span v-if="loading" class="inline-block animate-spin rounded-full h-4 w-4 border-b-2 border-white"></span>
                {{ loading ? '预览中...' : '🔍 开始预览' }}
              </button>
              <div class="mt-4 text-xs text-center text-slate-500">
                💡 预览将显示备份包中的服务配置和文件列表
              </div>
            </div>

            <div v-else>
              <h2 class="text-lg font-bold mb-4">📋 备份内容摘要</h2>
              
              <!-- Summary Grid -->
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

              <!-- Services List -->
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

              <!-- Warnings -->
              <div v-if="preview.manifest.errors.length > 0" class="mb-4 p-3 bg-amber-500/10 border border-amber-500/20 rounded-lg">
                <div class="text-sm font-medium text-amber-400 mb-1">备份时的警告</div>
                <div v-for="err in preview.manifest.errors" :key="err" class="text-xs text-amber-300">{{ err }}</div>
              </div>

              <!-- Action Button -->
              <div class="flex gap-3">
                <button
                  @click="handlePreview"
                  disabled
                  class="flex-1 py-3 bg-emerald-600 rounded-xl font-bold flex items-center justify-center gap-2 opacity-50 cursor-not-allowed"
                >
                  ✓ 预览完成
                </button>
                <button
                  @click="goToStep('verify')"
                  class="flex-1 py-3 bg-blue-600 hover:bg-blue-700 rounded-xl font-bold transition flex items-center justify-center gap-2"
                >
                  下一步：校验 →
                </button>
              </div>
            </div>
          </section>
        </Transition>

        <!-- Step 3: Verify Card -->
        <Transition name="step-fade">
          <section v-if="currentStep === 'verify'" class="bg-slate-900 border border-slate-800 rounded-xl p-6">
            <div v-if="verified === null">
              <h2 class="text-lg font-bold mb-4">🔒 校验文件完整性</h2>
              <button
                @click="handleVerify"
                :disabled="loading"
                class="w-full py-3 bg-emerald-600 hover:bg-emerald-700 rounded-xl font-bold transition disabled:opacity-50 flex items-center justify-center gap-2"
              >
                <span v-if="loading" class="inline-block animate-spin rounded-full h-4 w-4 border-b-2 border-white"></span>
                {{ loading ? '校验中...' : '🛡️ 开始校验' }}
              </button>
              <div class="mt-4 text-xs text-center text-slate-500">
                💡 校验将通过 SHA256 确保备份文件未被篡改
              </div>
            </div>

            <div v-else>
              <h2 class="text-lg font-bold mb-4">🔒 完整性校验结果</h2>
              <div v-if="verified" class="p-4 bg-emerald-500/10 border border-emerald-500/20 rounded-lg mb-4">
                <div class="flex items-center gap-2 text-emerald-400">
                  <svg xmlns="http://www.w3.org/2000/svg" class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                  </svg>
                  <span class="font-medium">SHA256 校验通过</span>
                </div>
                <p class="text-xs text-emerald-300/80 mt-2">备份包完整无损，可以安全恢复</p>
              </div>
              <div v-else class="p-4 bg-rose-500/10 border border-rose-500/20 rounded-lg mb-4">
                <div class="flex items-center gap-2 text-rose-400">
                  <svg xmlns="http://www.w3.org/2000/svg" class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                  <span class="font-medium">校验失败</span>
                </div>
                <p class="text-xs text-rose-300/80 mt-2">备份包可能已损坏或被篡改，请勿使用</p>
              </div>
              
              <!-- Next Step Button -->
              <button
                v-if="verified"
                @click="goToStep('restore')"
                class="w-full py-3 bg-blue-600 hover:bg-blue-700 rounded-xl font-bold transition flex items-center justify-center gap-2"
              >
                下一步：开始恢复 →
              </button>
            </div>
          </section>
        </Transition>

        <!-- Step 4: Restore Card -->
        <Transition name="step-fade">
          <section v-if="currentStep === 'restore'" class="bg-slate-900 border border-slate-800 rounded-xl p-6">
            <div v-if="!isStepCompleted('restore')">
              <h2 class="text-lg font-bold mb-4">🚀 开始恢复环境</h2>

              <!-- Restore Button -->
              <button
                @click="handleRestore"
                :disabled="!canRestore || restoring"
                class="w-full py-3 bg-emerald-600 hover:bg-emerald-700 rounded-xl font-bold transition disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
                :title="!canRestore ? '请先预览并校验备份文件完整性' : ''"
              >
                <span v-if="restoring" class="inline-block animate-spin rounded-full h-4 w-4 border-b-2 border-white"></span>
                {{ restoring ? '恢复中...' : '🚀 确认并开始恢复' }}
              </button>
              
              <!-- Helper text -->
              <div v-if="!canRestore && !restoring" class="mt-3 text-xs text-center text-slate-500">
                <span v-if="verified === false">❌ 备份文件校验失败，无法恢复</span>
              </div>
            </div>

            <!-- Success State -->
            <div v-else class="text-center py-8">
              <div class="inline-flex items-center justify-center w-16 h-16 bg-emerald-500/20 rounded-full mb-4">
                <svg xmlns="http://www.w3.org/2000/svg" class="w-8 h-8 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                </svg>
              </div>
              <h3 class="text-xl font-bold text-emerald-400 mb-2">环境恢复成功！</h3>
              <p class="text-sm text-slate-400">您可以重新启动 Docker 容器以应用新的配置</p>
            </div>
          </section>
        </Transition>

        <!-- Progress Bar (shown during restore) -->
        <Transition name="step-fade">
          <section v-if="progress && restoring" class="bg-slate-900 border border-slate-800 rounded-xl p-6">
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
        </Transition>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* Step transition animations */
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
