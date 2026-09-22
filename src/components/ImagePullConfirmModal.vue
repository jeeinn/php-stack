<script setup lang="ts">
/**
 * ImagePullConfirmModal.vue
 *
 * Phase 3: 用户点"应用配置"前，前端先调 check_service_images_presence 探测
 * 缺失的镜像。缺失列表传到这个弹窗，用户确认后：
 *   - emit('confirm', imageTags) — 前端调 pull_service_images 拉取
 *   - emit('cancel') — 用户放弃，回退到未应用状态
 *
 * 设计：每行 = 一个镜像 tag + 状态。Missing 项可拉取大小（best-effort），
 * 用户能一眼看到要拉什么、占多少磁盘。
 */
import { useI18n } from 'vue-i18n';
import { computed } from 'vue';
import type { ImagePresence } from '../types/env-config';

const { t: $t } = useI18n();

const props = defineProps<{
  open: boolean;
  /** 探测结果（带 status: present|missing） */
  presences: ImagePresence[];
  /** 加载标志：父组件发 pull 时置 true */
  pulling?: boolean;
  /** 单条进度（可选）：tag → 0-100 */
  progress?: Record<string, number>;
  /** 拉取进行中文案，如「正在拉取 php:8.2-fpm（1/3）」 */
  statusText?: string;
}>();

const emit = defineEmits<{
  (e: 'confirm', missingTags: string[]): void;
  (e: 'cancel'): void;
}>();

const missing = computed(() => props.presences.filter(p => p.status === 'missing'));
const present = computed(() => props.presences.filter(p => p.status === 'present'));

const summary = computed(() => {
  return $t('envConfig.pullConfirm.summary', {
    total: props.presences.length,
    missing: missing.value.length,
    present: present.value.length,
  });
});
</script>

<template>
  <div
    v-if="open"
    class="fixed inset-0 bg-black/70 backdrop-blur-sm flex items-center justify-center z-50 p-3 sm:p-4"
    @click.self="!pulling && emit('cancel')"
  >
    <div class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-700 rounded-xl w-full max-w-2xl mx-auto max-h-[90vh] flex flex-col shadow-2xl">
      <!-- Header -->
      <div class="flex justify-between items-start p-4 sm:p-5 border-b border-slate-200 dark:border-slate-700 shrink-0">
        <div>
          <h2 class="text-lg sm:text-xl font-bold text-slate-900 dark:text-slate-200">
            {{ $t('envConfig.pullConfirm.title') }}
          </h2>
          <p class="text-xs text-slate-500 dark:text-slate-400 mt-1 leading-relaxed">
            {{ $t('envConfig.pullConfirm.desc') }}
          </p>
        </div>
        <button
          v-if="!pulling"
          @click="emit('cancel')"
          class="text-slate-500 dark:text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 text-2xl leading-none"
        >
          &times;
        </button>
      </div>

      <!-- Summary banner -->
      <div
        v-if="pulling && statusText"
        class="px-4 sm:px-5 py-2.5 border-b border-slate-200 dark:border-slate-700 bg-blue-50 dark:bg-blue-950/30 text-blue-800 dark:text-blue-300 text-xs font-medium"
        data-testid="pull-status"
      >
        {{ statusText }}
      </div>
      <div
        v-else-if="missing.length > 0"
        class="px-4 sm:px-5 py-2.5 border-b border-slate-200 dark:border-slate-700 bg-blue-50 dark:bg-blue-950/30 text-blue-800 dark:text-blue-300 text-xs"
      >
        {{ summary }}
      </div>
      <div
        v-else
        class="px-4 sm:px-5 py-2.5 border-b border-slate-200 dark:border-slate-700 bg-emerald-50 dark:bg-emerald-950/30 text-emerald-700 dark:text-emerald-300 text-xs"
      >
        {{ $t('envConfig.pullConfirm.allPresent') }}
      </div>

      <!-- Body: 镜像清单 -->
      <div class="flex-1 overflow-y-auto p-4 sm:p-5 space-y-2">
        <!-- 缺失的：需用户确认 -->
        <div v-for="p in missing" :key="p.tag" class="flex items-center justify-between gap-3 px-3 py-2 rounded-lg border border-blue-200 dark:border-blue-800 bg-blue-50 dark:bg-blue-950/20">
          <div class="flex items-center gap-2 min-w-0">
            <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4 text-blue-500 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01M5.07 19h13.86c1.54 0 2.5-1.67 1.73-3L13.73 4c-.77-1.33-2.69-1.33-3.46 0L3.34 16c-.77 1.33.19 3 1.73 3z" />
            </svg>
            <code class="text-xs font-mono text-slate-700 dark:text-slate-200 truncate">{{ p.tag }}</code>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <span v-if="pulling && progress && progress[p.tag] != null" class="text-[10px] text-blue-500 font-mono">{{ progress[p.tag] }}%</span>
            <span class="text-[10px] px-2 py-0.5 ui-tag-blue rounded font-medium">
              {{ $t('envConfig.pullConfirm.toPull') }}
            </span>
          </div>
        </div>

        <!-- 已存在的 -->
        <div v-for="p in present" :key="p.tag" class="flex items-center justify-between gap-3 px-3 py-2 rounded-lg border border-emerald-200 dark:border-emerald-800 bg-emerald-50 dark:bg-emerald-950/20">
          <div class="flex items-center gap-2 min-w-0">
            <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4 text-emerald-500 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
            </svg>
            <code class="text-xs font-mono text-slate-700 dark:text-slate-200 truncate">{{ p.tag }}</code>
          </div>
          <span class="text-[10px] text-slate-500 dark:text-slate-500 font-mono">{{ p.size || '—' }}</span>
        </div>

        <!-- 提示文字 -->
        <div class="text-xs text-slate-500 dark:text-slate-500 mt-4 px-1 leading-relaxed">
          {{ $t('envConfig.pullConfirm.tip', { action: $t('envConfig.startEnv') }) }}
        </div>
      </div>

      <!-- Footer -->
      <div class="p-4 sm:p-5 border-t border-slate-200 dark:border-slate-700 flex flex-col sm:flex-row justify-end gap-3 shrink-0">
        <button
          @click="emit('cancel')"
          :disabled="pulling"
          class="px-5 py-2 ui-btn-secondary rounded-lg font-medium transition disabled:opacity-50"
        >
          {{ $t('common.cancel') }}
        </button>
        <button
          @click="emit('confirm', missing.map(p => p.tag))"
          :disabled="pulling || missing.length === 0"
          class="px-5 py-2 ui-btn-primary rounded-lg font-medium transition disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
        >
          <svg v-if="pulling" class="w-4 h-4 animate-spin" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v4a4 4 0 00-4 4H4z"></path>
          </svg>
          <span>{{ pulling ? (statusText || $t('envConfig.pullConfirm.pulling')) : $t('envConfig.pullConfirm.confirmPull', { count: missing.length }) }}</span>
        </button>
      </div>
    </div>
  </div>
</template>
