<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { open as openDirectory } from '@tauri-apps/plugin-dialog';
import type { SiteEntry } from '../types/env-config';
import { normalizeMountPath, normalizeError, relativePublicDir } from '../api';
import { showToast } from '../composables/useToast';
import CustomSelect from './CustomSelect.vue';

export type SelectOption = { value: string; label: string };

const props = defineProps<{
  /** 是否显示。勿命名为 open：会与 dialog 的 open 导入在模板里撞名。 */
  visible: boolean;
  /** 打开时的初始站点；关闭时可为 null。 */
  site: SiteEntry | null;
  /** true = 添加，false = 编辑 */
  isNew: boolean;
  phpOptions: SelectOption[];
  nginxOptions: SelectOption[];
}>();

const emit = defineEmits<{
  (e: 'cancel'): void;
  (e: 'confirm', site: SiteEntry): void;
}>();

const { t } = useI18n();

const draft = ref<SiteEntry | null>(null);

function repairHostPath(path: string): string {
  const normalized = (path || '').replace(/\\/g, '/');
  if (normalized.startsWith('./')) {
    const rest = normalized.slice(2);
    if (/^[A-Za-z]:/.test(rest) || rest.startsWith('/')) return rest;
  }
  return normalized;
}

function normalizePublicDir(raw: string | undefined): string {
  let path = (raw || '').replace(/\\/g, '/').trim().replace(/^\/+|\/+$/g, '');
  if (path.startsWith('./')) path = path.slice(2);
  if (!path || path === '.') return '';
  return path;
}

function siteDerived(site: SiteEntry) {
  const id = site.id.replace(/[^a-zA-Z0-9_-]/g, '') || 'main';
  const slug = id.replace(/[^a-zA-Z0-9]/g, '').toUpperCase() || 'SITE';
  const deferred = !!site.deferred;
  const noted = repairHostPath(site.noted_host_path || '');
  const host = deferred ? '' : repairHostPath(site.host_path);
  return {
    id,
    host_path: host,
    public_dir: normalizePublicDir(site.public_dir),
    env_key: `SITE_${slug}`,
    container_path: `/sites/${id}`,
    deferred,
    noted_host_path: deferred ? noted || repairHostPath(site.host_path) : noted,
  };
}

const derived = computed(() => (draft.value ? siteDerived(draft.value) : null));

const nginxRoot = computed(() => {
  if (!derived.value) return '';
  const base = (derived.value.container_path || '').replace(/\/$/, '');
  return derived.value.public_dir ? `${base}/${derived.value.public_dir}` : base;
});

const dialogTitle = computed(() =>
  props.isNew ? t('envConfig.sites.dialogTitle') : t('envConfig.sites.dialogTitleEdit'),
);

watch(
  () => [props.visible, props.site] as const,
  ([isVisible, site]) => {
    if (!isVisible || !site) {
      draft.value = null;
      return;
    }
    draft.value = { ...site };
  },
  { immediate: true },
);

async function pickHostPath(current: string): Promise<string | null> {
  const selected = await openDirectory({
    directory: true,
    multiple: false,
    defaultPath: current && !current.startsWith('.') ? current : undefined,
  });
  if (!selected || Array.isArray(selected)) return null;
  try {
    return await normalizeMountPath(selected);
  } catch (e) {
    showToast(normalizeError(e), 'error');
    return null;
  }
}

async function browseSiteDir() {
  if (!draft.value) return;
  const picked = await pickHostPath(draft.value.host_path || draft.value.noted_host_path || '');
  if (picked) {
    draft.value.host_path = picked;
    draft.value.deferred = false;
    draft.value.noted_host_path = '';
  }
}

function toggleDeferred(deferred: boolean) {
  if (!draft.value) return;
  if (deferred) {
    if (!draft.value.noted_host_path && draft.value.host_path) {
      draft.value.noted_host_path = repairHostPath(draft.value.host_path);
    }
    draft.value.host_path = '';
    draft.value.deferred = true;
  } else {
    draft.value.deferred = false;
    if (!draft.value.host_path && draft.value.noted_host_path) {
      draft.value.host_path = draft.value.noted_host_path;
    }
    draft.value.noted_host_path = '';
  }
}

function parkUnderWww() {
  if (!draft.value) return;
  const id = draft.value.id.replace(/[^a-zA-Z0-9_-]/g, '') || 'main';
  draft.value.host_path = `./www/${id}`;
  draft.value.deferred = false;
  draft.value.noted_host_path = '';
}

async function browsePublicDir() {
  if (!draft.value) return;
  if (!draft.value.host_path.trim()) {
    showToast(t('envConfig.sites.mountFirst'), 'error');
    return;
  }
  const selected = await openDirectory({
    directory: true,
    multiple: false,
    defaultPath: draft.value.host_path.startsWith('.') ? undefined : draft.value.host_path,
  });
  if (!selected || Array.isArray(selected)) return;
  try {
    draft.value.public_dir = await relativePublicDir(draft.value.host_path, selected);
  } catch (e) {
    showToast(normalizeError(e), 'error');
  }
}

function handleConfirm() {
  if (!draft.value) return;
  const d = siteDerived(draft.value);
  emit('confirm', {
    ...draft.value,
    ...d,
  });
}
</script>

<template>
  <div
    v-if="visible && draft"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4"
    data-testid="site-edit-dialog"
    @click.self="emit('cancel')"
  >
    <div
      class="bg-white dark:bg-slate-900 rounded-xl p-6 max-w-lg w-full border border-slate-200 dark:border-slate-700 shadow-2xl max-h-[90vh] overflow-y-auto space-y-4"
    >
      <div class="flex items-start justify-between gap-3">
        <h2 class="text-xl font-bold text-slate-900 dark:text-slate-200">{{ dialogTitle }}</h2>
        <button
          type="button"
          class="shrink-0 text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 text-xl leading-none px-1"
          :aria-label="$t('common.cancel')"
          @click="emit('cancel')"
        >
          ×
        </button>
      </div>

      <div class="flex flex-wrap items-center justify-between gap-2">
        <span
          v-if="draft.deferred"
          class="text-[11px] px-2 py-0.5 rounded-full bg-amber-500/15 text-amber-700 dark:text-amber-300 border border-amber-500/30"
        >{{ $t('envConfig.sites.deferredBadge') }}</span>
        <label class="flex items-center gap-2 text-xs text-slate-600 dark:text-slate-400 ml-auto">
          <input
            type="checkbox"
            class="accent-blue-500"
            :checked="!!draft.deferred"
            @change="(event) => toggleDeferred((event.target as HTMLInputElement).checked)"
          />
          {{ $t('envConfig.sites.deferToggle') }}
        </label>
      </div>
      <p v-if="draft.deferred" class="text-[11px] text-amber-600 dark:text-amber-400">
        {{ $t('envConfig.sites.deferredHint') }}
      </p>
      <p
        v-if="draft.deferred && draft.noted_host_path"
        class="text-[11px] text-slate-500 font-mono"
      >
        {{ $t('envConfig.sites.notedPath', { path: draft.noted_host_path }) }}
      </p>

      <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <div>
          <label class="block text-xs text-slate-600 dark:text-slate-400 mb-1">{{ $t('envConfig.sites.id') }}</label>
          <input
            v-model="draft.id"
            type="text"
            class="w-full bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-900 dark:text-slate-200 outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>
        <div>
          <label class="block text-xs text-slate-600 dark:text-slate-400 mb-1">{{ $t('envConfig.sites.serverName') }}</label>
          <input
            v-model="draft.server_name"
            type="text"
            class="w-full bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-900 dark:text-slate-200 outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>
        <div>
          <label class="block text-xs text-slate-600 dark:text-slate-400 mb-1">{{ $t('envConfig.sites.php') }}</label>
          <CustomSelect
            :modelValue="draft.php_service"
            :options="phpOptions"
            @change="(value) => (draft!.php_service = String(value))"
          />
        </div>
        <div>
          <label class="block text-xs text-slate-600 dark:text-slate-400 mb-1">{{ $t('envConfig.sites.nginx') }}</label>
          <CustomSelect
            :modelValue="draft.nginx_service"
            :options="nginxOptions"
            @change="(value) => (draft!.nginx_service = String(value))"
          />
        </div>
      </div>

      <div v-if="!draft.deferred" class="space-y-3">
        <div>
          <label class="block text-xs text-slate-600 dark:text-slate-400 mb-1">{{ $t('envConfig.sites.hostPath') }}</label>
          <div class="flex flex-wrap gap-2">
            <input
              v-model="draft.host_path"
              type="text"
              placeholder="./www"
              class="flex-1 min-w-0 bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-900 dark:text-slate-200 outline-none focus:ring-2 focus:ring-blue-500"
            />
            <button type="button" class="shrink-0 px-3 py-2 text-sm ui-btn-soft rounded-lg" @click="browseSiteDir">
              {{ $t('envConfig.sites.browse') }}
            </button>
            <button type="button" class="shrink-0 px-3 py-2 text-sm ui-btn-soft rounded-lg" @click="parkUnderWww">
              {{ $t('envConfig.sites.parkWww') }}
            </button>
          </div>
          <p class="text-[11px] text-slate-500 mt-1">{{ $t('envConfig.sites.hostPathHint') }}</p>
        </div>
        <div>
          <label class="block text-xs text-slate-600 dark:text-slate-400 mb-1">{{ $t('envConfig.sites.publicDir') }}</label>
          <div class="flex gap-2">
            <input
              v-model="draft.public_dir"
              type="text"
              placeholder="public"
              class="flex-1 min-w-0 bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-900 dark:text-slate-200 outline-none focus:ring-2 focus:ring-blue-500"
            />
            <button type="button" class="shrink-0 px-3 py-2 text-sm ui-btn-soft rounded-lg" @click="browsePublicDir">
              {{ $t('envConfig.sites.browse') }}
            </button>
          </div>
          <p class="text-[11px] text-slate-500 mt-1">{{ $t('envConfig.sites.publicDirHint') }}</p>
        </div>
      </div>
      <div v-else class="flex gap-2">
        <button type="button" class="px-3 py-2 text-sm ui-btn-soft rounded-lg" @click="browseSiteDir">
          {{ $t('envConfig.sites.browse') }}
        </button>
        <button type="button" class="px-3 py-2 text-sm ui-btn-soft rounded-lg" @click="parkUnderWww">
          {{ $t('envConfig.sites.parkWww') }}
        </button>
      </div>

      <p v-if="derived" class="text-[11px] text-slate-500 font-mono">
        {{ derived.env_key }} → {{ derived.container_path }} ·
        {{ $t('envConfig.sites.nginxRoot', { root: nginxRoot }) }}
      </p>

      <div class="flex gap-3 pt-2">
        <button
          type="button"
          class="flex-1 px-4 py-2 bg-slate-100 dark:bg-slate-700 rounded-lg"
          @click="emit('cancel')"
        >
          {{ $t('common.cancel') }}
        </button>
        <button
          type="button"
          class="flex-1 px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg"
          @click="handleConfirm"
        >
          {{ $t('common.save') }}
        </button>
      </div>
    </div>
  </div>
</template>
