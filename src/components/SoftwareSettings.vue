<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke } from '@tauri-apps/api/core';
import type { VersionMappings, VersionInfo, ServiceTypeLower } from '../types/env-config';
import { showToast } from '../composables/useToast';
import { showConfirm } from '../composables/useConfirmDialog';

const { t } = useI18n();

const versionMappings = ref<VersionMappings | null>(null);
const loading = ref(false);
const selectedService = ref<ServiceTypeLower>('mysql');

// 编辑对话框状态
const showEditDialog = ref(false);
const editingVersion = ref<VersionInfo | null>(null);
const editTag = ref('');
const editDescription = ref('');

// 服务类型标签映射
const serviceLabels: Record<ServiceTypeLower, string> = {
  php: 'PHP',
  mysql: 'MySQL',
  redis: 'Redis',
  nginx: 'Nginx'
};

// 加载版本映射数据
async function loadVersionMappings() {
  loading.value = true;
  
  try {
    const data = await invoke<VersionMappings>('get_version_mappings');
    // 标记有用户覆盖的版本
    versionMappings.value = data;
  } catch (e) {
    showToast(t('software.toast.loadFailed', { error: e }), 'error');
  } finally {
    loading.value = false;
  }
}

// 获取当前服务的版本列表
function getCurrentVersions(): VersionInfo[] {
  if (!versionMappings.value) return [];
  return versionMappings.value[selectedService.value] || [];
}

// 复制镜像名称
async function copyImageName(fullName: string) {
  try {
    await navigator.clipboard.writeText(fullName);
    showToast(t('software.toast.copied'), 'success');
  } catch (e) {
    showToast(t('software.toast.copyFailed'), 'error');
  }
}

// 打开编辑对话框
function openEditDialog(version: VersionInfo) {
  editingVersion.value = version;
  editTag.value = version.image_tag;
  editDescription.value = version.description || '';
  showEditDialog.value = true;
}

// 保存用户覆盖
async function saveOverride() {
  if (!editingVersion.value) return;
  
  loading.value = true;
  
  try {
    await invoke('save_user_override', {
      serviceType: selectedService.value,
      id: editingVersion.value.id,
      imageTag: editTag.value,
      description: editDescription.value || undefined
    });
    
    showToast(t('software.toast.saved'), 'success');
    showEditDialog.value = false;
    editingVersion.value = null;
    
    // 重新加载数据
    await loadVersionMappings();
  } catch (e) {
    showToast(t('software.toast.saveFailed', { error: e }), 'error');
  } finally {
    loading.value = false;
  }
}

// 删除用户覆盖
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
    await invoke('remove_user_override', {
      serviceType: selectedService.value,
      id: version.id
    });
    
    showToast(t('software.toast.deleted'), 'success');
    
    // 重新加载数据
    await loadVersionMappings();
  } catch (e) {
    showToast(t('software.toast.deleteFailed', { error: e }), 'error');
  } finally {
    loading.value = false;
  }
}

// 重置所有覆盖
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
    await invoke('reset_all_overrides');
    
    showToast(t('software.toast.resetDone'), 'success');
    
    // 重新加载数据
    await loadVersionMappings();
  } catch (e) {
    showToast(t('software.toast.resetFailed', { error: e }), 'error');
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  loadVersionMappings();
});
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden">
    <header class="mb-6 flex justify-between items-start">
      <div>
        <p class="text-slate-400 text-sm">{{ $t('software.subtitle') }}</p>
      </div>
      <button
        @click="resetAllOverrides"
        class="px-4 py-2 bg-rose-600 hover:bg-rose-700 text-white rounded-lg transition text-sm"
      >
        {{ $t('software.resetAll') }}
      </button>
    </header>

    <!-- Loading State -->
    <div v-if="loading" class="flex-1 flex items-center justify-center">
      <div class="text-center">
        <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-500 mx-auto mb-4"></div>
        <p class="text-slate-400">{{ $t('common.loading') }}</p>
      </div>
    </div>

    <!-- Content -->
    <div v-else-if="versionMappings" class="flex-1 flex flex-col min-h-0">
      <!-- Service Tabs -->
      <div class="flex gap-2 mb-4 border-b border-slate-700 pb-2 flex-shrink-0">
        <button
          v-for="(label, service) in serviceLabels"
          :key="service"
          @click="selectedService = service as ServiceTypeLower"
          :class="[
            'px-4 py-1.5 rounded-lg font-medium transition text-xs sm:text-sm',
            selectedService === service
              ? 'bg-blue-600 text-white'
              : 'bg-slate-800 text-slate-300 hover:bg-slate-700'
          ]"
        >
          {{ label }}
        </button>
      </div>

      <!-- Version Table -->
      <div class="flex-1 overflow-auto min-h-0">
        <div class="overflow-x-auto -mx-3 sm:mx-0">
          <table class="w-full text-left border-collapse min-w-[900px]">
            <thead class="sticky top-0 bg-slate-900 z-10">
              <tr class="border-b border-slate-700">
                <th class="py-3 px-3 text-slate-400 font-medium whitespace-nowrap text-sm min-w-[120px]">{{ $t('software.table.name') }}</th>
                <th class="py-3 px-3 text-slate-400 font-medium whitespace-nowrap text-sm min-w-[200px]">{{ $t('software.table.image') }}</th>
                <th class="py-3 px-3 text-slate-400 font-medium whitespace-nowrap text-sm min-w-[100px]">{{ $t('software.table.configDir') }}</th>
                <th class="py-3 px-3 text-slate-400 font-medium whitespace-nowrap text-sm min-w-[80px]">{{ $t('software.table.status') }}</th>
                <th class="py-3 px-3 text-slate-400 font-medium whitespace-nowrap text-sm min-w-[150px]">{{ $t('software.table.notes') }}</th>
                <th class="py-3 px-3 text-slate-400 font-medium whitespace-nowrap text-sm sticky right-0 bg-slate-900 z-20 w-auto">{{ $t('software.table.actions') }}</th>
              </tr>
            </thead>
            <tbody>
              <tr 
                v-for="version in getCurrentVersions()" 
                :key="version.id"
                class="border-b border-slate-800 hover:bg-slate-800/50 transition"
              >
                <td class="py-3 px-3">
                  <code class="bg-slate-800 px-2 py-1 rounded text-sm">{{ version.display_name }}</code>
                </td>
                <td class="py-3 px-3">
                  <code 
                    @click="copyImageName(version.image_tag)"
                    class="bg-slate-800 px-2 py-1 rounded text-xs block cursor-pointer hover:bg-slate-700 transition truncate"
                    :title="'点击复制: ' + version.image_tag"
                  >
                    {{ version.image_tag }}
                  </code>
                  <span v-if="version.has_user_override" class="ml-1 text-xs text-yellow-400">({{ $t('mirror.status.custom') }})</span>
                </td>
                <td class="py-3 px-3">
                  <code class="bg-slate-800 px-2 py-1 rounded text-xs">{{ version.service_dir }}</code>
                </td>
                <td class="py-3 px-3">
                  <span 
                    :class="version.eol ? 'bg-rose-500/20 text-rose-400' : 'bg-green-500/20 text-green-400'"
                    class="px-2 py-1 rounded text-xs font-medium whitespace-nowrap"
                  >
                    {{ version.eol ? $t('software.status.eol') : $t('software.status.active') }}
                  </span>
                </td>
                <td class="py-3 px-3 text-slate-400 text-sm truncate" :title="version.description || ''">
                  {{ version.description || '-' }}
                </td>
                <td class="py-3 px-3 sticky right-0 bg-slate-900 z-10 whitespace-nowrap">
                  <div class="flex items-center gap-2">
                    <button
                      @click="openEditDialog(version)"
                      class="px-3 py-1.5 bg-yellow-600 hover:bg-yellow-700 text-white rounded text-xs transition"
                      title=""
                    >
                      {{ $t('common.edit') }}
                    </button>
                    <button
                      v-if="version.has_user_override"
                      @click="removeOverride(version)"
                      class="px-3 py-1.5 bg-rose-600 hover:bg-rose-700 text-white rounded text-xs transition"
                      title=""
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
      <div class="mt-4 p-4 bg-slate-800/50 rounded-lg text-sm text-slate-400">
        <p>{{ $t('software.hints.title') }}</p>
        <ul class="list-disc list-inside mt-2 space-y-1">
          <li>{{ $t('software.hints.editCustom') }}</li>
          <li>{{ $t('software.hints.reapply') }}</li>
          <li>{{ $t('software.hints.eolWarning') }}</li>
        </ul>
      </div>
    </div>

    <!-- Edit Dialog -->
    <div v-if="showEditDialog" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div class="bg-slate-900 rounded-xl p-6 max-w-md w-full mx-4 border border-slate-700">
        <h2 class="text-xl font-bold mb-4">{{ $t('software.editDialog.title') }}</h2>
        
        <div class="space-y-4">
          <div>
            <label class="block text-sm text-slate-400 mb-2">{{ $t('software.editDialog.versionLabel') }}</label>
            <input
              v-if="editingVersion"
              :value="editingVersion.display_name"
              disabled
              class="w-full px-3 py-2 bg-slate-800 border border-slate-700 rounded text-slate-400"
            />
          </div>
          
          <div>
            <label class="block text-sm text-slate-400 mb-2">{{ $t('software.editDialog.imageLabel') }} <span class="text-rose-400">*</span></label>
            <input
              v-model="editTag"
              :placeholder="$t('software.editDialog.imagePlaceholder')" 
              class="w-full px-3 py-2 bg-slate-800 border border-slate-700 rounded focus:border-blue-500 focus:outline-none"
            />
          </div>
          
          <div>
            <label class="block text-sm text-slate-400 mb-2">{{ $t('software.editDialog.descLabel') }}</label>
            <textarea
              v-model="editDescription"
              :placeholder="$t('software.editDialog.descPlaceholder')" 
              rows="3"
              class="w-full px-3 py-2 bg-slate-800 border border-slate-700 rounded focus:border-blue-500 focus:outline-none resize-none"
            ></textarea>
          </div>
        </div>
        
        <div class="flex gap-3 mt-6">
          <button
            @click="showEditDialog = false"
            class="flex-1 px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition"
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
  </div>
</template>
