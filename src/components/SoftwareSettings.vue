<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { VersionMappings, VersionInfo, ServiceType } from '../types/version-mapping';

const versionMappings = ref<VersionMappings | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);
const successMsg = ref<string | null>(null);
const selectedService = ref<ServiceType>('mysql');

// 编辑对话框状态
const showEditDialog = ref(false);
const editingVersion = ref<VersionInfo | null>(null);
const editTag = ref('');
const editDescription = ref('');

// 服务类型标签映射
const serviceLabels: Record<ServiceType, string> = {
  php: 'PHP',
  mysql: 'MySQL',
  redis: 'Redis',
  nginx: 'Nginx'
};

// 加载版本映射数据
async function loadVersionMappings() {
  loading.value = true;
  error.value = null;
  
  try {
    const data = await invoke<VersionMappings>('get_version_mappings');
    // 标记有用户覆盖的版本
    versionMappings.value = data;
  } catch (e) {
    error.value = `加载版本映射失败: ${e}`;
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
    successMsg.value = '已复制到剪贴板！';
    setTimeout(() => { successMsg.value = null; }, 2000);
  } catch (e) {
    error.value = '复制失败';
  }
}

// 打开编辑对话框
function openEditDialog(version: VersionInfo) {
  editingVersion.value = version;
  editTag.value = version.tag;
  editDescription.value = version.description || '';
  showEditDialog.value = true;
}

// 保存用户覆盖
async function saveOverride() {
  if (!editingVersion.value) return;
  
  loading.value = true;
  error.value = null;
  
  try {
    await invoke('save_user_override', {
      serviceType: selectedService.value,
      version: editingVersion.value.version,
      tag: editTag.value,
      description: editDescription.value || undefined
    });
    
    successMsg.value = '保存成功！重新应用配置后生效。';
    showEditDialog.value = false;
    editingVersion.value = null;
    
    // 重新加载数据
    await loadVersionMappings();
  } catch (e) {
    error.value = `保存失败: ${e}`;
  } finally {
    loading.value = false;
  }
}

// 删除用户覆盖
async function removeOverride(version: VersionInfo) {
  if (!confirm(`确定要删除 ${version.version} 的自定义配置吗？`)) return;
  
  loading.value = true;
  error.value = null;
  
  try {
    await invoke('remove_user_override', {
      serviceType: selectedService.value,
      version: version.version
    });
    
    successMsg.value = '已恢复为默认配置';
    
    // 重新加载数据
    await loadVersionMappings();
  } catch (e) {
    error.value = `删除失败: ${e}`;
  } finally {
    loading.value = false;
  }
}

// 重置所有覆盖
async function resetAllOverrides() {
  if (!confirm('确定要重置所有自定义配置吗？此操作不可撤销。')) return;
  
  loading.value = true;
  error.value = null;
  
  try {
    await invoke('reset_all_overrides');
    
    successMsg.value = '已重置所有自定义配置';
    
    // 重新加载数据
    await loadVersionMappings();
  } catch (e) {
    error.value = `重置失败: ${e}`;
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
        <h1 class="text-3xl font-bold">软件设置</h1>
        <p class="text-slate-400 text-sm mt-1">管理 Docker 镜像版本映射配置</p>
      </div>
      <button
        @click="resetAllOverrides"
        class="px-4 py-2 bg-rose-600 hover:bg-rose-700 text-white rounded-lg transition text-sm"
      >
        🔄 重置所有自定义
      </button>
    </header>

    <!-- Error / Success Alert -->
    <div v-if="error" class="mb-4 p-4 bg-rose-500/10 border border-rose-500/20 rounded-xl text-rose-400 text-sm">
      {{ error }}
    </div>
    <div v-if="successMsg" class="mb-4 p-4 bg-green-500/10 border border-green-500/20 rounded-xl text-green-400 text-sm">
      {{ successMsg }}
    </div>

    <!-- Loading State -->
    <div v-if="loading" class="flex-1 flex items-center justify-center">
      <div class="text-center">
        <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-500 mx-auto mb-4"></div>
        <p class="text-slate-400">加载中...</p>
      </div>
    </div>

    <!-- Content -->
    <div v-else-if="versionMappings" class="flex-1 flex flex-col">
      <!-- Service Tabs -->
      <div class="flex gap-2 mb-6 border-b border-slate-700 pb-2">
        <button
          v-for="(label, service) in serviceLabels"
          :key="service"
          @click="selectedService = service as ServiceType"
          :class="[
            'px-4 py-2 rounded-lg font-medium transition',
            selectedService === service
              ? 'bg-blue-600 text-white'
              : 'bg-slate-800 text-slate-300 hover:bg-slate-700'
          ]"
        >
          {{ label }}
        </button>
      </div>

      <!-- Version Table -->
      <div class="flex-1 overflow-auto">
        <table class="w-full text-left border-collapse">
          <thead>
            <tr class="border-b border-slate-700">
              <th class="pb-3 px-2 text-slate-400 font-medium">应用名称</th>
              <th class="pb-3 px-2 text-slate-400 font-medium">版本号</th>
              <th class="pb-3 px-2 text-slate-400 font-medium">Docker 镜像标签</th>
              <th class="pb-3 px-2 text-slate-400 font-medium">完整镜像名</th>
              <th class="pb-3 px-2 text-slate-400 font-medium">状态</th>
              <th class="pb-3 px-2 text-slate-400 font-medium">备注</th>
              <th class="pb-3 px-2 text-slate-400 font-medium">操作</th>
            </tr>
          </thead>
          <tbody>
            <tr 
              v-for="version in getCurrentVersions()" 
              :key="version.version"
              class="border-b border-slate-800 hover:bg-slate-800/50 transition"
            >
              <td class="py-3 px-2">{{ serviceLabels[selectedService] }}</td>
              <td class="py-3 px-2">
                <code class="bg-slate-800 px-2 py-1 rounded text-sm">{{ version.version }}</code>
              </td>
              <td class="py-3 px-2">
                <code class="bg-slate-800 px-2 py-1 rounded text-sm">{{ version.tag }}</code>
                <span v-if="version.has_user_override" class="ml-2 text-xs text-yellow-400">(自定义)</span>
              </td>
              <td class="py-3 px-2">
                <code class="bg-slate-800 px-2 py-1 rounded text-sm">{{ version.full_name }}</code>
              </td>
              <td class="py-3 px-2">
                <span 
                  :class="version.eol ? 'bg-rose-500/20 text-rose-400' : 'bg-green-500/20 text-green-400'"
                  class="px-2 py-1 rounded text-xs font-medium"
                >
                  {{ version.eol ? '⚠️ EOL' : '✅ 活跃' }}
                </span>
              </td>
              <td class="py-3 px-2 text-slate-400 text-sm">
                {{ version.description || '-' }}
              </td>
              <td class="py-3 px-2">
                <div class="flex gap-2">
                  <button
                    @click="copyImageName(version.full_name)"
                    class="px-3 py-1 bg-blue-600 hover:bg-blue-700 text-white rounded text-xs transition"
                  >
                    📋 复制
                  </button>
                  <button
                    @click="openEditDialog(version)"
                    class="px-3 py-1 bg-yellow-600 hover:bg-yellow-700 text-white rounded text-xs transition"
                  >
                    ✏️ 编辑
                  </button>
                  <button
                    v-if="version.has_user_override"
                    @click="removeOverride(version)"
                    class="px-3 py-1 bg-rose-600 hover:bg-rose-700 text-white rounded text-xs transition"
                  >
                    🗑️ 删除
                  </button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- Footer Info -->
      <div class="mt-4 p-4 bg-slate-800/50 rounded-lg text-sm text-slate-400">
        <p>💡 提示：</p>
        <ul class="list-disc list-inside mt-2 space-y-1">
          <li>点击"编辑"可以自定义 Docker 镜像标签</li>
          <li>修改后需要重新应用环境配置才能生效</li>
          <li>EOL 版本表示已停止维护，建议使用活跃版本</li>
        </ul>
      </div>
    </div>

    <!-- Edit Dialog -->
    <div v-if="showEditDialog" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div class="bg-slate-900 rounded-xl p-6 max-w-md w-full mx-4 border border-slate-700">
        <h2 class="text-xl font-bold mb-4">编辑镜像标签</h2>
        
        <div class="space-y-4">
          <div>
            <label class="block text-sm text-slate-400 mb-2">版本号</label>
            <input
              v-if="editingVersion"
              :value="editingVersion.version"
              disabled
              class="w-full px-3 py-2 bg-slate-800 border border-slate-700 rounded text-slate-400"
            />
          </div>
          
          <div>
            <label class="block text-sm text-slate-400 mb-2">Docker 镜像标签 <span class="text-rose-400">*</span></label>
            <input
              v-model="editTag"
              placeholder="例如: 8.4-lts"
              class="w-full px-3 py-2 bg-slate-800 border border-slate-700 rounded focus:border-blue-500 focus:outline-none"
            />
          </div>
          
          <div>
            <label class="block text-sm text-slate-400 mb-2">备注说明</label>
            <textarea
              v-model="editDescription"
              placeholder="可选，描述这个自定义标签的用途"
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
            取消
          </button>
          <button
            @click="saveOverride"
            :disabled="!editTag.trim()"
            class="flex-1 px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-slate-600 disabled:cursor-not-allowed text-white rounded-lg transition"
          >
            保存
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
