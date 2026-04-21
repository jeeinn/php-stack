<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { MergedMirrorCategory, MirrorSourceOption } from '../types/env-config';

const categories = ref<MergedMirrorCategory[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);
const successMsg = ref<string | null>(null);
const selectedCategory = ref('docker_registry');

// 类别标签映射
const categoryLabels: Record<string, string> = {
  docker_registry: 'Docker Registry',
  apt: 'APT / Debian',
  composer: 'Composer',
  npm: 'NPM',
  github_proxy: 'GitHub Proxy'
};

// 编辑对话框状态
const showEditDialog = ref(false);
const editingOption = ref<MirrorSourceOption | null>(null);
const editValue = ref('');
const editDescription = ref('');
const isCustomEdit = ref(false);

// 加载镜像源列表
async function loadMirrorList() {
  loading.value = true;
  error.value = null;
  
  try {
    const data = await invoke<MergedMirrorCategory[]>('get_merged_mirror_list');
    categories.value = data;
  } catch (e) {
    error.value = `加载镜像源列表失败: ${e}`;
  } finally {
    loading.value = false;
  }
}

// 获取当前类别的选项列表
function getCurrentOptions(): MirrorSourceOption[] {
  const category = categories.value.find(c => c.category_id === selectedCategory.value);
  return category?.options || [];
}

// 测试连接
async function testConnection(option: MirrorSourceOption) {
  if (!option.value) return;
  
  loading.value = true;
  try {
    const result = await invoke<boolean>('test_mirror', { url: option.value });
    if (result) {
      successMsg.value = `✅ ${option.name} 连接成功！`;
    } else {
      error.value = `❌ ${option.name} 连接失败，请检查网络或镜像源地址`;
    }
    setTimeout(() => { successMsg.value = null; error.value = null; }, 3000);
  } catch (e) {
    error.value = `测试失败: ${e}`;
  } finally {
    loading.value = false;
  }
}

// 选择镜像源
async function selectMirror(option: MirrorSourceOption) {
  loading.value = true;
  error.value = null;
  
  try {
    await invoke('save_selected_mirror_option', {
      categoryId: selectedCategory.value,
      optionId: option.id
    });
    
    successMsg.value = `已选择 ${option.name}`;
    setTimeout(() => { successMsg.value = null; }, 2000);
    
    // 重新加载数据
    await loadMirrorList();
  } catch (e) {
    error.value = `保存失败: ${e}`;
  } finally {
    loading.value = false;
  }
}

// 打开编辑对话框（自定义）
function openCustomEdit() {
  editingOption.value = null;
  isCustomEdit.value = true;
  editValue.value = '';
  editDescription.value = '';
  showEditDialog.value = true;
}

// 打开编辑对话框（已有选项）
function openEditDialog(option: MirrorSourceOption) {
  editingOption.value = option;
  isCustomEdit.value = false;
  editValue.value = option.value;
  editDescription.value = option.description || '';
  showEditDialog.value = true;
}

// 保存自定义镜像源
async function saveCustomMirror() {
  if (!editValue.value.trim()) {
    error.value = '请输入镜像源地址';
    return;
  }
  
  loading.value = true;
  error.value = null;
  
  try {
    await invoke('save_user_mirror_category', {
      categoryId: selectedCategory.value,
      source: editValue.value.trim(),
      description: editDescription.value || undefined
    });
    
    successMsg.value = '自定义镜像源已保存！';
    showEditDialog.value = false;
    
    // 重新加载数据
    await loadMirrorList();
  } catch (e) {
    error.value = `保存失败: ${e}`;
  } finally {
    loading.value = false;
  }
}

// 删除自定义镜像源
async function removeCustomMirror() {
  if (!confirm(`确定要删除自定义镜像源吗？`)) return;
  
  loading.value = true;
  error.value = null;
  
  try {
    await invoke('remove_user_mirror_category', {
      categoryId: selectedCategory.value
    });
    
    successMsg.value = '已恢复为默认配置';
    showEditDialog.value = false;
    
    // 重新加载数据
    await loadMirrorList();
  } catch (e) {
    error.value = `删除失败: ${e}`;
  } finally {
    loading.value = false;
  }
}

// 重置所有自定义
async function resetAllOverrides() {
  if (!confirm('确定要重置所有自定义镜像源配置吗？此操作不可撤销。')) return;
  
  loading.value = true;
  error.value = null;
  
  try {
    await invoke('reset_all_mirror_overrides');
    
    successMsg.value = '已重置所有自定义配置';
    
    // 重新加载数据
    await loadMirrorList();
  } catch (e) {
    error.value = `重置失败: ${e}`;
  } finally {
    loading.value = false;
  }
}

// 应用配置（更新 .env）
async function applyConfig() {
  loading.value = true;
  error.value = null;
  
  try {
    for (const category of categories.value) {
      await invoke('update_single_mirror', {
        category: category.category_id,
        source: category.current_value
      });
    }
    
    successMsg.value = '镜像源配置已应用到 .env 文件！';
    setTimeout(() => { successMsg.value = null; }, 3000);
  } catch (e) {
    error.value = `应用配置失败: ${e}`;
  } finally {
    loading.value = false;
  }
}

// 复制镜像源地址
async function copyUrl(url: string) {
  try {
    await navigator.clipboard.writeText(url);
    successMsg.value = '已复制到剪贴板！';
    setTimeout(() => { successMsg.value = null; }, 2000);
  } catch (e) {
    error.value = '复制失败';
  }
}

onMounted(() => {
  loadMirrorList();
});
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden">
    <header class="mb-6 flex justify-between items-start">
      <div>
        <h1 class="text-3xl font-bold">镜像源管理</h1>
        <p class="text-slate-400 text-sm mt-1">统一管理 Docker、APT、Composer、NPM 镜像源</p>
      </div>
      <div class="flex gap-2">
        <button
          @click="resetAllOverrides"
          class="px-4 py-2 bg-rose-600 hover:bg-rose-700 text-white rounded-lg transition text-sm"
        >
          🔄 重置所有自定义
        </button>
        <button
          @click="applyConfig"
          :disabled="loading"
          class="px-5 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg font-medium transition disabled:opacity-50 text-sm"
        >
          {{ loading ? '应用中...' : '应用配置' }}
        </button>
      </div>
    </header>

    <!-- Error / Success Alert -->
    <div v-if="error" class="mb-4 p-4 bg-rose-500/10 border border-rose-500/20 rounded-xl text-rose-400 text-sm">
      {{ error }}
    </div>
    <div v-if="successMsg" class="mb-4 p-4 bg-green-500/10 border border-green-500/20 rounded-xl text-green-400 text-sm">
      {{ successMsg }}
    </div>

    <!-- Loading State -->
    <div v-if="loading && categories.length === 0" class="flex-1 flex items-center justify-center">
      <div class="text-center">
        <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-500 mx-auto mb-4"></div>
        <p class="text-slate-400">加载中...</p>
      </div>
    </div>

    <!-- Content -->
    <div v-else-if="categories.length > 0" class="flex-1 flex flex-col min-h-0">
      <!-- Category Tabs -->
      <div class="flex gap-2 mb-4 border-b border-slate-700 pb-2 flex-shrink-0">
        <button
          v-for="category in categories"
          :key="category.category_id"
          @click="selectedCategory = category.category_id"
          :class="[
            'px-4 py-2 rounded-lg font-medium transition',
            selectedCategory === category.category_id
              ? 'bg-blue-600 text-white'
              : 'bg-slate-800 text-slate-300 hover:bg-slate-700'
          ]"
        >
          {{ categoryLabels[category.category_id] || category.category_id }}
          <span v-if="category.has_user_override" class="ml-1 text-xs text-yellow-300">✏️</span>
        </button>
      </div>

      <!-- Add Custom Button -->
      <div class="mb-3 flex-shrink-0">
        <button
          @click="openCustomEdit"
          class="px-4 py-2 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg text-sm transition"
        >
          ➕ 新增自定义镜像源
        </button>
      </div>

      <!-- Options Table -->
      <div class="flex-1 overflow-auto min-h-0">
        <div class="overflow-x-auto">
          <table class="w-full text-left border-collapse min-w-[600px]">
            <thead class="sticky top-0 bg-slate-900 z-10">
              <tr class="border-b border-slate-700">
                <th class="pb-3 px-3 text-slate-400 font-medium whitespace-nowrap text-sm">镜像源名称</th>
                <th class="pb-3 px-3 text-slate-400 font-medium whitespace-nowrap text-sm">地址</th>
                <th class="pb-3 px-3 text-slate-400 font-medium whitespace-nowrap text-sm">描述</th>
                <th class="pb-3 px-3 text-slate-400 font-medium whitespace-nowrap text-sm">状态</th>
                <th class="pb-3 px-3 text-slate-400 font-medium whitespace-nowrap text-sm">操作</th>
              </tr>
            </thead>
            <tbody>
              <tr 
                v-for="option in getCurrentOptions()" 
                :key="option.id"
                :class="[
                  'border-b border-slate-800 hover:bg-slate-800/50 transition',
                  categories.find(c => c.category_id === selectedCategory)?.selected_id === option.id 
                    ? 'bg-blue-600/10' 
                    : ''
                ]"
              >
                <td class="py-3 px-3 text-sm">
                  <span class="font-medium">{{ option.name }}</span>
                  <span 
                    v-if="categories.find(c => c.category_id === selectedCategory)?.selected_id === option.id" 
                    class="ml-2 text-xs text-blue-400"
                  >
                    (当前)
                  </span>
                  <span 
                    v-if="option.id === 'custom'" 
                    class="ml-2 text-xs text-yellow-400"
                  >
                    (自定义)
                  </span>
                </td>
                <td class="py-3 px-3">
                  <code class="bg-slate-800 px-2 py-1 rounded text-xs break-all">{{ option.value || '-' }}</code>
                </td>
                <td class="py-3 px-3 text-slate-400 text-xs max-w-[200px] truncate" :title="option.description || ''">
                  {{ option.description || '-' }}
                </td>
                <td class="py-3 px-3">
                  <span 
                    v-if="option.value"
                    class="px-2 py-1 rounded text-xs font-medium bg-blue-500/20 text-blue-400"
                  >
                    可用
                  </span>
                  <span 
                    v-else
                    class="px-2 py-1 rounded text-xs font-medium bg-slate-500/20 text-slate-400"
                  >
                    官方默认
                  </span>
                </td>
                <td class="py-3 px-3">
                  <div class="flex gap-1.5">
                    <button
                      @click="selectMirror(option)"
                      :class="[
                        'px-3 py-1.5 rounded text-xs transition whitespace-nowrap',
                        categories.find(c => c.category_id === selectedCategory)?.selected_id === option.id
                          ? 'bg-blue-600 text-white cursor-default'
                          : 'bg-slate-700 hover:bg-slate-600 text-white'
                      ]"
                      :disabled="categories.find(c => c.category_id === selectedCategory)?.selected_id === option.id"
                      title="选择此镜像源"
                    >
                      选择
                    </button>
                    <button
                      v-if="option.value"
                      @click="testConnection(option)"
                      class="px-3 py-1.5 bg-green-600 hover:bg-green-700 text-white rounded text-xs transition whitespace-nowrap"
                      title="测试连接"
                    >
                      🔗
                    </button>
                    <button
                      @click="copyUrl(option.value)"
                      class="px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-white rounded text-xs transition whitespace-nowrap"
                      title="复制地址"
                    >
                      📋
                    </button>
                    <button
                      v-if="option.id === 'custom' || option.value"
                      @click="openEditDialog(option)"
                      class="px-3 py-1.5 bg-yellow-600 hover:bg-yellow-700 text-white rounded text-xs transition whitespace-nowrap"
                      title="编辑"
                    >
                      ✏️
                    </button>
                    <button
                      v-if="option.id === 'custom'"
                      @click="removeCustomMirror"
                      class="px-3 py-1.5 bg-rose-600 hover:bg-rose-700 text-white rounded text-xs transition whitespace-nowrap"
                      title="删除自定义"
                    >
                      🗑️
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
        <p>💡 提示：</p>
        <ul class="list-disc list-inside mt-2 space-y-1">
          <li>选择镜像源后点击"应用配置"保存到 .env 文件</li>
          <li>自定义镜像源会被保存到 .user_mirror_config.json</li>
          <li>点击"测试连接"验证镜像源是否可用</li>
        </ul>
      </div>
    </div>

    <!-- Edit/Custom Dialog -->
    <div v-if="showEditDialog" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div class="bg-slate-900 rounded-xl p-6 max-w-md w-full mx-4 border border-slate-700">
        <h2 class="text-xl font-bold mb-4">
          {{ isCustomEdit ? '新增自定义镜像源' : '编辑镜像源' }}
        </h2>
        
        <div class="space-y-4">
          <div>
            <label class="block text-sm text-slate-400 mb-2">
              镜像源地址 <span class="text-rose-400">*</span>
            </label>
            <input
              v-model="editValue"
              placeholder="例如: https://registry.npmmirror.com"
              class="w-full px-3 py-2 bg-slate-800 border border-slate-700 rounded focus:border-blue-500 focus:outline-none"
            />
          </div>
          
          <div>
            <label class="block text-sm text-slate-400 mb-2">备注说明</label>
            <textarea
              v-model="editDescription"
              placeholder="可选，描述这个镜像源的用途"
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
            v-if="!isCustomEdit && editingOption?.id === 'custom'"
            @click="removeCustomMirror"
            class="px-4 py-2 bg-rose-600 hover:bg-rose-700 text-white rounded-lg transition"
          >
            删除
          </button>
          <button
            @click="isCustomEdit ? saveCustomMirror() : null"
            :disabled="!editValue.trim()"
            class="flex-1 px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-slate-600 disabled:cursor-not-allowed text-white rounded-lg transition"
          >
            {{ isCustomEdit ? '保存' : '更新' }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
