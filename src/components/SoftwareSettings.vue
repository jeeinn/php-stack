<script setup lang="ts">
import { ref, onMounted, computed, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import {
  getVersionMappings,
  saveUserOverride,
  addCustomVersion,
  removeUserOverride,
  resetAllOverrides as resetAllOverridesApi,
  normalizeError,
} from '../api';
import type { VersionMappings, VersionInfo, ServiceTypeLower } from '../types/env-config';
import { showToast } from '../composables/useToast';
import { showConfirm } from '../composables/useConfirmDialog';
import UiTabs from './UiTabs.vue';

const { t } = useI18n();

const versionMappings = ref<VersionMappings | null>(null);
const loading = ref(false);
const selectedService = ref<ServiceTypeLower>('mysql');

// 编辑对话框状态
const showEditDialog = ref(false);
const editingVersion = ref<VersionInfo | null>(null);
const editTag = ref('');
const editDescription = ref('');

// 新增对话框状态
const showAddDialog = ref(false);
const addService = ref<ServiceTypeLower>('mysql');
const addId = ref('');
const addDisplayName = ref('');
const addImageTag = ref('');
const addServiceDir = ref('');
const addDefaultPort = ref(0);
const addShowPort = ref(true);
const addEol = ref(false);
const addDescription = ref('');
const addIdTouched = ref(false);

const serviceLabels: Record<ServiceTypeLower, string> = {
  php: 'PHP',
  mysql: 'MySQL',
  redis: 'Redis',
  nginx: 'Nginx'
};

const defaultPorts: Record<ServiceTypeLower, number> = {
  php: 9000,
  mysql: 3306,
  redis: 6379,
  nginx: 80,
};

const serviceTabItems = computed(() =>
  (Object.keys(serviceLabels) as ServiceTypeLower[]).map((service) => ({
    id: service,
    label: serviceLabels[service],
  })),
);

const suggestedServiceDirs = computed(() => {
  if (!versionMappings.value) return [];
  const versions = versionMappings.value[addService.value] || [];
  return [...new Set(versions.map((v) => v.service_dir).filter(Boolean))];
});

const addFormValid = computed(() => {
  return (
    addId.value.trim().length > 0 &&
    /^[A-Za-z0-9_-]+$/.test(addId.value.trim()) &&
    addDisplayName.value.trim().length > 0 &&
    addImageTag.value.trim().length > 0 &&
    addServiceDir.value.trim().length > 0 &&
    Number.isFinite(addDefaultPort.value) &&
    addDefaultPort.value > 0
  );
});

function applyAddServiceDefaults(service: ServiceTypeLower) {
  addDefaultPort.value = defaultPorts[service];
  addShowPort.value = service !== 'php';
  const versions = versionMappings.value?.[service] || [];
  const dirs = [...new Set(versions.map((v) => v.service_dir).filter(Boolean))];
  addServiceDir.value = dirs[0] || service;
  if (!addIdTouched.value) {
    addId.value = suggestIdFromDisplayName(addDisplayName.value, service);
  }
}

async function loadVersionMappings() {
  loading.value = true;

  try {
    const data = await getVersionMappings();
    versionMappings.value = data;
  } catch (e) {
    showToast(t('software.toast.loadFailed', { error: normalizeError(e) }), 'error');
  } finally {
    loading.value = false;
  }
}

function getCurrentVersions(): VersionInfo[] {
  if (!versionMappings.value) return [];
  return versionMappings.value[selectedService.value] || [];
}

async function copyImageName(fullName: string) {
  try {
    await navigator.clipboard.writeText(fullName);
    showToast(t('software.toast.copied'), 'success');
  } catch (e) {
    showToast(t('software.toast.copyFailed'), 'error');
  }
}

function openEditDialog(version: VersionInfo) {
  editingVersion.value = version;
  editTag.value = version.image_tag;
  editDescription.value = version.description || '';
  showEditDialog.value = true;
}

async function saveOverride() {
  if (!editingVersion.value) return;

  loading.value = true;

  try {
    await saveUserOverride(
      selectedService.value,
      editingVersion.value.id,
      editTag.value,
      editDescription.value || undefined,
    );

    showToast(t('software.toast.saved'), 'success');
    showEditDialog.value = false;
    editingVersion.value = null;

    await loadVersionMappings();
  } catch (e) {
    showToast(t('software.toast.saveFailed', { error: normalizeError(e) }), 'error');
  } finally {
    loading.value = false;
  }
}

/** 从显示名推测 id，如 "Redis 8.4" → redis84 */
function suggestIdFromDisplayName(displayName: string, service: ServiceTypeLower): string {
  const digits = displayName.replace(/[^\d.]/g, '').replace(/\./g, '');
  if (!digits) return '';
  return `${service}${digits}`;
}

function openAddDialog() {
  addIdTouched.value = false;
  addService.value = selectedService.value;
  addDisplayName.value = '';
  addId.value = '';
  addImageTag.value = '';
  addDescription.value = '';
  addEol.value = false;
  applyAddServiceDefaults(addService.value);
  showAddDialog.value = true;
}

watch(addDisplayName, (name) => {
  if (addIdTouched.value) return;
  addId.value = suggestIdFromDisplayName(name, addService.value);
});

watch(addService, (service) => {
  applyAddServiceDefaults(service);
});

async function saveCustom() {
  if (!addFormValid.value) return;

  loading.value = true;

  try {
    await addCustomVersion({
      serviceType: addService.value,
      id: addId.value.trim(),
      displayName: addDisplayName.value.trim(),
      imageTag: addImageTag.value.trim(),
      serviceDir: addServiceDir.value.trim(),
      defaultPort: addDefaultPort.value,
      showPort: addShowPort.value,
      eol: addEol.value,
      description: addDescription.value.trim() || undefined,
    });

    showToast(t('software.toast.added'), 'success');
    showAddDialog.value = false;
    selectedService.value = addService.value;
    await loadVersionMappings();
  } catch (e) {
    showToast(t('software.toast.addFailed', { error: normalizeError(e) }), 'error');
  } finally {
    loading.value = false;
  }
}

async function removeOverride(version: VersionInfo) {
  const confirmed = await showConfirm({
    title: t('software.confirm.deleteTitle'),
    message: t('software.confirm.deleteMessage', { name: version.display_name }),
    confirmText: t('common.delete'),
    type: 'danger'
  });

  if (!confirmed) return;

  loading.value = true;

  try {
    await removeUserOverride(selectedService.value, version.id);

    showToast(t('software.toast.deleted'), 'success');

    await loadVersionMappings();
  } catch (e) {
    showToast(t('software.toast.deleteFailed', { error: normalizeError(e) }), 'error');
  } finally {
    loading.value = false;
  }
}

async function resetAllOverrides() {
  const confirmed = await showConfirm({
    title: t('software.confirm.resetTitle'),
    message: t('software.confirm.resetMessage'),
    confirmText: t('common.reset'),
    type: 'danger'
  });

  if (!confirmed) return;

  loading.value = true;

  try {
    await resetAllOverridesApi();

    showToast(t('software.toast.resetDone'), 'success');

    await loadVersionMappings();
  } catch (e) {
    showToast(t('software.toast.resetFailed', { error: normalizeError(e) }), 'error');
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  loadVersionMappings();
});
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-200 transition-colors duration-300">
    <header class="mb-4 sm:mb-6 flex flex-col sm:flex-row justify-between items-start gap-3">
      <div>
        <p class="text-slate-500 dark:text-slate-400 text-xs sm:text-sm">{{ $t('software.subtitle') }}</p>
      </div>
      <div class="flex gap-2 w-full sm:w-auto">
        <button
          @click="openAddDialog"
          class="flex-1 sm:flex-none px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg transition text-sm"
        >
          {{ $t('software.addCustom') }}
        </button>
        <button
          @click="resetAllOverrides"
          class="flex-1 sm:flex-none ui-btn-danger px-4 py-2 rounded-lg transition text-sm"
        >
          {{ $t('software.resetAll') }}
        </button>
      </div>
    </header>

    <!-- Loading State -->
    <div v-if="loading && !versionMappings" class="flex-1 flex items-center justify-center">
      <div class="text-center">
        <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-500 mx-auto mb-4"></div>
        <p class="text-slate-500 dark:text-slate-400">{{ $t('common.loading') }}</p>
      </div>
    </div>

    <!-- Content -->
    <div v-else-if="versionMappings" class="flex-1 flex flex-col min-h-0">
      <!-- Service Tabs -->
      <div class="mb-3 sm:mb-4 flex-shrink-0">
        <UiTabs v-model="selectedService" :items="serviceTabItems" size="sm" />
      </div>

      <!-- Version Table -->
      <div class="flex-1 overflow-auto min-h-0">
        <div class="overflow-x-auto -mx-3 sm:mx-0">
          <table class="w-full text-left border-collapse min-w-[900px]">
            <thead class="sticky top-0 bg-white dark:bg-slate-900 z-10">
              <tr class="border-b border-slate-200 dark:border-slate-700">
                <th class="py-3 px-3 text-slate-600 dark:text-slate-400 font-medium whitespace-nowrap text-sm min-w-[120px]">{{ $t('software.table.name') }}</th>
                <th class="py-3 px-3 text-slate-600 dark:text-slate-400 font-medium whitespace-nowrap text-sm min-w-[200px]">{{ $t('software.table.image') }}</th>
                <th class="py-3 px-3 text-slate-600 dark:text-slate-400 font-medium whitespace-nowrap text-sm min-w-[100px]">{{ $t('software.table.configDir') }}</th>
                <th class="py-3 px-3 text-slate-600 dark:text-slate-400 font-medium whitespace-nowrap text-sm min-w-[80px]">{{ $t('software.table.status') }}</th>
                <th class="py-3 px-3 text-slate-600 dark:text-slate-400 font-medium whitespace-nowrap text-sm min-w-[150px]">{{ $t('software.table.notes') }}</th>
                <th class="py-3 px-3 text-slate-600 dark:text-slate-400 font-medium whitespace-nowrap text-sm sticky right-0 bg-white dark:bg-slate-900 z-20 w-auto">{{ $t('software.table.actions') }}</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="version in getCurrentVersions()"
                :key="version.id"
                class="border-b border-slate-200 dark:border-slate-800 hover:bg-slate-50 dark:hover:bg-slate-800/50 transition"
              >
                <td class="py-3 px-3">
                  <code class="bg-slate-100 dark:bg-slate-800 px-2 py-1 rounded text-sm text-slate-700 dark:text-slate-300">{{ version.display_name }}</code>
                </td>
                <td class="py-3 px-3">
                  <code
                    @click="copyImageName(version.image_tag)"
                    class="bg-slate-100 dark:bg-slate-800 px-2 py-1 rounded text-xs block cursor-pointer hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-300 transition truncate"
                    :title="t('software.toast.copyTooltip', { tag: version.image_tag })"
                  >
                    {{ version.image_tag }}
                  </code>
                  <span v-if="version.has_user_override" class="ml-1 text-xs text-slate-500 dark:text-slate-400">{{ $t('mirror.status.custom') }}</span>
                </td>
                <td class="py-3 px-3">
                  <code class="bg-slate-100 dark:bg-slate-800 px-2 py-1 rounded text-xs text-slate-700 dark:text-slate-300">{{ version.service_dir }}</code>
                </td>
                <td class="py-3 px-3">
                  <span
                    :class="version.eol ? 'bg-rose-500/20 text-rose-400' : 'bg-green-500/20 text-green-400'"
                    class="px-2 py-1 rounded text-xs font-medium whitespace-nowrap"
                  >
                    {{ version.eol ? $t('software.status.eol') : $t('software.status.active') }}
                  </span>
                </td>
                <td class="py-3 px-3 text-slate-600 dark:text-slate-400 text-sm truncate" :title="version.description || ''">
                  {{ version.description || '-' }}
                </td>
                <td class="py-3 px-3 sticky right-0 bg-white dark:bg-slate-900 z-10 whitespace-nowrap">
                  <div class="flex items-center gap-2">
                    <button
                      @click="openEditDialog(version)"
                      class="px-3 py-1.5 ui-btn-secondary rounded text-xs transition"
                    >
                      {{ $t('common.edit') }}
                    </button>
                    <button
                      v-if="version.has_user_override"
                      @click="removeOverride(version)"
                      class="px-3 py-1.5 ui-btn-danger rounded text-xs transition"
                    >
                      {{ $t('common.delete') }}
                    </button>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      <!-- Footer Info -->
      <div class="mt-4 p-4 bg-white dark:bg-slate-800/50 rounded-lg text-sm text-slate-600 dark:text-slate-400 border border-slate-200 dark:border-slate-700">
        <p>{{ $t('software.hints.title') }}</p>
        <ul class="list-disc list-inside mt-2 space-y-1">
          <li>{{ $t('software.hints.editCustom', { action: $t('common.edit') }) }}</li>
          <li>{{ $t('software.hints.addCustom', { action: $t('software.addCustom') }) }}</li>
          <li>{{ $t('software.hints.reapply') }}</li>
          <li>{{ $t('software.hints.eolWarning') }}</li>
        </ul>
      </div>
    </div>

    <!-- Edit Dialog -->
    <div v-if="showEditDialog" class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50">
      <div class="bg-white dark:bg-slate-900 rounded-xl p-6 max-w-md w-full mx-4 border border-slate-200 dark:border-slate-700 shadow-2xl">
        <h2 class="text-xl font-bold mb-4 text-slate-900 dark:text-slate-200">{{ $t('software.editDialog.title') }}</h2>

        <div class="space-y-4">
          <div>
            <label class="block text-sm text-slate-600 dark:text-slate-400 mb-2">{{ $t('software.editDialog.versionLabel') }}</label>
            <input
              v-if="editingVersion"
              :value="editingVersion.display_name"
              disabled
              class="w-full px-3 py-2 bg-slate-100 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded text-slate-500 dark:text-slate-500 cursor-not-allowed"
            />
          </div>

          <div>
            <label class="block text-sm text-slate-600 dark:text-slate-400 mb-2">{{ $t('software.editDialog.imageLabel') }} <span class="text-rose-500 dark:text-rose-400">*</span></label>
            <input
              v-model="editTag"
              :placeholder="$t('software.editDialog.imagePlaceholder')"
              class="w-full px-3 py-2 bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:outline-none"
            />
          </div>

          <div>
            <label class="block text-sm text-slate-600 dark:text-slate-400 mb-2">{{ $t('software.editDialog.descLabel') }}</label>
            <textarea
              v-model="editDescription"
              :placeholder="$t('software.editDialog.descPlaceholder')"
              rows="3"
              class="w-full px-3 py-2 bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:outline-none resize-none"
            ></textarea>
          </div>
        </div>

        <div class="flex gap-3 mt-6">
          <button
            @click="showEditDialog = false"
            class="flex-1 px-4 py-2 bg-slate-100 dark:bg-slate-700 hover:bg-slate-200 dark:hover:bg-slate-600 text-slate-700 dark:text-slate-300 rounded-lg transition"
          >
            {{ $t('common.cancel') }}
          </button>
          <button
            @click="saveOverride"
            :disabled="!editTag.trim()"
            class="flex-1 px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-slate-600 disabled:cursor-not-allowed text-white rounded-lg transition"
          >
            {{ $t('common.save') }}
          </button>
        </div>
      </div>
    </div>

    <!-- Add Dialog -->
    <div v-if="showAddDialog" class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50">
      <div class="bg-white dark:bg-slate-900 rounded-xl p-6 max-w-lg w-full mx-4 border border-slate-200 dark:border-slate-700 shadow-2xl max-h-[90vh] overflow-y-auto">
        <h2 class="text-xl font-bold mb-4 text-slate-900 dark:text-slate-200">{{ $t('software.addDialog.title') }}</h2>

        <div class="space-y-4">
          <div>
            <label class="block text-sm text-slate-600 dark:text-slate-400 mb-2">{{ $t('software.addDialog.categoryLabel') }} <span class="text-rose-500">*</span></label>
            <select
              v-model="addService"
              class="w-full px-3 py-2 bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:outline-none"
            >
              <option v-for="item in serviceTabItems" :key="item.id" :value="item.id">
                {{ item.label }}
              </option>
            </select>
          </div>

          <div>
            <label class="block text-sm text-slate-600 dark:text-slate-400 mb-2">{{ $t('software.addDialog.displayNameLabel') }} <span class="text-rose-500">*</span></label>
            <input
              v-model="addDisplayName"
              :placeholder="$t('software.addDialog.displayNamePlaceholder')"
              class="w-full px-3 py-2 bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:outline-none"
            />
          </div>

          <div>
            <label class="block text-sm text-slate-600 dark:text-slate-400 mb-2">{{ $t('software.addDialog.idLabel') }} <span class="text-rose-500">*</span></label>
            <input
              v-model="addId"
              @input="addIdTouched = true"
              :placeholder="$t('software.addDialog.idPlaceholder')"
              class="w-full px-3 py-2 bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:outline-none font-mono text-sm"
            />
            <p class="mt-1 text-xs text-slate-500">{{ $t('software.addDialog.idHint') }}</p>
          </div>

          <div>
            <label class="block text-sm text-slate-600 dark:text-slate-400 mb-2">{{ $t('software.addDialog.imageLabel') }} <span class="text-rose-500">*</span></label>
            <input
              v-model="addImageTag"
              :placeholder="$t('software.addDialog.imagePlaceholder')"
              class="w-full px-3 py-2 bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:outline-none font-mono text-sm"
            />
          </div>

          <div>
            <label class="block text-sm text-slate-600 dark:text-slate-400 mb-2">{{ $t('software.addDialog.serviceDirLabel') }} <span class="text-rose-500">*</span></label>
            <input
              v-model="addServiceDir"
              list="service-dir-suggestions"
              class="w-full px-3 py-2 bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:outline-none font-mono text-sm"
            />
            <datalist id="service-dir-suggestions">
              <option v-for="dir in suggestedServiceDirs" :key="dir" :value="dir" />
            </datalist>
            <p class="mt-1 text-xs text-slate-500">{{ $t('software.addDialog.serviceDirHint') }}</p>
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div>
              <label class="block text-sm text-slate-600 dark:text-slate-400 mb-2">{{ $t('software.addDialog.defaultPortLabel') }} <span class="text-rose-500">*</span></label>
              <input
                v-model.number="addDefaultPort"
                type="number"
                min="1"
                max="65535"
                class="w-full px-3 py-2 bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:outline-none"
              />
            </div>
            <div class="flex flex-col justify-end gap-2 pb-1">
              <label class="inline-flex items-center gap-2 text-sm text-slate-600 dark:text-slate-400">
                <input v-model="addShowPort" type="checkbox" class="rounded border-slate-400" />
                {{ $t('software.addDialog.showPortLabel') }}
              </label>
              <label class="inline-flex items-center gap-2 text-sm text-slate-600 dark:text-slate-400">
                <input v-model="addEol" type="checkbox" class="rounded border-slate-400" />
                {{ $t('software.addDialog.eolLabel') }}
              </label>
            </div>
          </div>

          <div>
            <label class="block text-sm text-slate-600 dark:text-slate-400 mb-2">{{ $t('software.addDialog.descLabel') }}</label>
            <textarea
              v-model="addDescription"
              :placeholder="$t('software.addDialog.descPlaceholder')"
              rows="2"
              class="w-full px-3 py-2 bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:outline-none resize-none"
            ></textarea>
          </div>
        </div>

        <div class="flex gap-3 mt-6">
          <button
            @click="showAddDialog = false"
            class="flex-1 px-4 py-2 bg-slate-100 dark:bg-slate-700 hover:bg-slate-200 dark:hover:bg-slate-600 text-slate-700 dark:text-slate-300 rounded-lg transition"
          >
            {{ $t('common.cancel') }}
          </button>
          <button
            @click="saveCustom"
            :disabled="!addFormValid"
            class="flex-1 px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-slate-600 disabled:cursor-not-allowed text-white rounded-lg transition"
          >
            {{ $t('common.save') }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
