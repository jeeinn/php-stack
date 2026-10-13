<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { open as openDirectory } from '@tauri-apps/plugin-dialog';
import type { SiteEntry, SiteMountChoice, SitePathOverride } from '../types/env-config';
import { normalizeMountPath, normalizeError } from '../api';
import { showToast } from '../composables/useToast';

const props = defineProps<{
  /** 是否显示。勿命名为 open：会与 @tauri-apps/plugin-dialog 的 open 导入在模板里撞名。 */
  visible: boolean;
  sites: SiteEntry[];
  /** 启动闸门报错里点名的站点 id；为空则展示全部非暂缓且可能缺目录的站点。 */
  focusIds?: string[];
}>();

const emit = defineEmits<{
  (e: 'cancel'): void;
  (e: 'confirm', overrides: SitePathOverride[]): void;
}>();

const { t } = useI18n();

interface Row {
  site: SiteEntry;
  choice: SiteMountChoice;
  host_path: string;
}

const rows = ref<Row[]>([]);

function suggestedWwwPath(siteId: string): string {
  const id = siteId.replace(/[^a-zA-Z0-9_-]/g, '') || 'main';
  return `./www/${id}`;
}

function buildRows(sites: SiteEntry[], focusIds: string[] | undefined): Row[] {
  const focus = new Set(focusIds ?? []);
  let list =
    focus.size > 0
      ? sites.filter(
          s => focus.has(s.id) || (!s.deferred && !s.host_path?.trim()),
        )
      : sites.filter(s => !s.deferred);

  // 配置读不到时，仍按报错 id 合成占位行，避免空壳弹窗
  if (list.length === 0 && focus.size > 0) {
    list = [...focus].map(id => ({
      id,
      server_name: id,
      host_path: '',
      env_key: `SITE_${id.replace(/[^a-zA-Z0-9]/g, '').toUpperCase() || 'SITE'}`,
      container_path: `/sites/${id}`,
      nginx_service: '',
      php_service: '',
      public_dir: '',
      deferred: false,
      noted_host_path: '',
    }));
  }

  return list.map(site => ({
    site,
    choice: 'defer' as SiteMountChoice,
    host_path: site.host_path || site.noted_host_path || '',
  }));
}

watch(
  () => [props.visible, props.sites, props.focusIds] as const,
  ([isVisible, sites, focusIds]) => {
    if (!isVisible) {
      rows.value = [];
      return;
    }
    rows.value = buildRows(sites, focusIds);
  },
  { immediate: true, deep: true },
);

const canConfirm = computed(
  () =>
    rows.value.length > 0 &&
    rows.value.every(row => row.choice !== 'map' || !!row.host_path.trim()),
);

function setChoice(index: number, choice: SiteMountChoice) {
  const row = rows.value[index];
  if (!row) return;
  row.choice = choice;
  if (choice === 'park') {
    row.host_path = suggestedWwwPath(row.site.id);
  } else if (choice === 'defer') {
    row.host_path = '';
  }
}

async function browse(index: number) {
  const selected = await openDirectory({ directory: true, multiple: false });
  if (!selected || Array.isArray(selected)) return;
  try {
    const path = await normalizeMountPath(selected);
    const row = rows.value[index];
    if (row) {
      row.host_path = path;
      row.choice = 'map';
    }
  } catch (e) {
    showToast(normalizeError(e), 'error');
  }
}

function handleConfirm() {
  if (rows.value.length === 0) {
    showToast(t('dashboard.siteMount.emptyRows'), 'error');
    return;
  }
  if (!canConfirm.value) {
    showToast(
      t('dashboard.siteMount.pathRequired', {
        action: t('dashboard.siteMount.choiceMap'),
      }),
      'error',
    );
    return;
  }
  const overrides: SitePathOverride[] = rows.value.map(row => ({
    env_key: row.site.env_key,
    host_path: row.choice === 'park' ? suggestedWwwPath(row.site.id) : row.host_path,
    choice: row.choice,
  }));
  emit('confirm', overrides);
}
</script>

<template>
  <div
    v-if="visible"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
    data-testid="site-mount-dialog"
    @click.self="emit('cancel')"
  >
    <div class="w-full max-w-lg rounded-xl bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-700 shadow-xl p-5 space-y-4">
      <div class="flex items-start justify-between gap-3">
        <h2 class="text-lg font-bold text-slate-900 dark:text-slate-100">{{ $t('dashboard.siteMount.title') }}</h2>
        <button
          type="button"
          class="shrink-0 text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 text-xl leading-none px-1"
          :aria-label="$t('common.cancel')"
          @click="emit('cancel')"
        >
          ×
        </button>
      </div>
      <p class="text-sm text-slate-600 dark:text-slate-400">{{ $t('dashboard.siteMount.message') }}</p>

      <p v-if="rows.length === 0" class="text-sm text-amber-600 dark:text-amber-400">
        {{ $t('dashboard.siteMount.emptyRows') }}
      </p>

      <div
        v-for="(row, index) in rows"
        :key="row.site.env_key"
        class="border border-slate-200 dark:border-slate-700 rounded-lg p-3 space-y-2"
      >
        <div class="text-sm font-medium text-slate-800 dark:text-slate-200">
          {{ row.site.server_name }} · {{ row.site.id }}
        </div>
        <p v-if="row.site.noted_host_path || row.site.host_path" class="text-[11px] text-slate-500 font-mono">
          {{ row.site.noted_host_path || row.site.host_path }}
        </p>
        <div class="flex flex-wrap gap-3 text-xs text-slate-600 dark:text-slate-400">
          <label class="flex items-center gap-1.5">
            <input type="radio" class="accent-blue-500" :name="`mount-${row.site.env_key}`" :checked="row.choice === 'map'" @change="setChoice(index, 'map')" />
            {{ $t('dashboard.siteMount.choiceMap') }}
          </label>
          <label class="flex items-center gap-1.5">
            <input type="radio" class="accent-blue-500" :name="`mount-${row.site.env_key}`" :checked="row.choice === 'park'" @change="setChoice(index, 'park')" />
            {{ $t('dashboard.siteMount.choicePark') }}
          </label>
          <label class="flex items-center gap-1.5">
            <input type="radio" class="accent-blue-500" :name="`mount-${row.site.env_key}`" :checked="row.choice === 'defer'" @change="setChoice(index, 'defer')" />
            {{ $t('dashboard.siteMount.choiceDefer') }}
          </label>
        </div>
        <div v-if="row.choice !== 'defer'" class="flex gap-2">
          <input
            v-model="row.host_path"
            type="text"
            :readonly="row.choice === 'park'"
            class="flex-1 bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-xs font-mono"
          />
          <button
            v-if="row.choice === 'map'"
            type="button"
            class="shrink-0 px-3 py-2 text-xs ui-btn-soft rounded-lg"
            @click="browse(index)"
          >
            {{ $t('dashboard.siteMount.browse') }}
          </button>
        </div>
      </div>

      <div class="flex gap-3 pt-1">
        <button type="button" class="flex-1 py-2.5 ui-btn-soft rounded-xl font-medium" @click="emit('cancel')">
          {{ $t('dashboard.siteMount.cancel') }}
        </button>
        <button
          type="button"
          class="flex-1 py-2.5 ui-btn-primary rounded-xl font-bold disabled:opacity-50"
          :disabled="!canConfirm"
          @click="handleConfirm"
        >
          {{ $t('dashboard.siteMount.apply') }}
        </button>
      </div>
    </div>
  </div>
</template>
