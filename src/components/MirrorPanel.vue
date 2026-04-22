<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { MergedMirrorCategory, MirrorSourceOption } from '../types/env-config';
import { showToast } from '../composables/useToast';
import { showConfirm } from '../composables/useConfirmDialog';

const categories = ref<MergedMirrorCategory[]>([]);
const loading = ref(false);
const selectedCategory = ref('apt');
const testingOptions = ref<Set<string>>(new Set()); // 跟踪正在测试的选项

// 类别标签映射
const categoryLabels: Record<string, string> = {
  apt: 'APT / Debian',
  composer: 'Composer',
  npm: 'NPM',
  github_proxy: 'GitHub Proxy',
  docker_registry: 'Docker Registry'
};

// 类别排序（Docker Registry 排在最后）
const categoryOrder = ['apt', 'composer', 'npm', 'github_proxy', 'docker_registry'];

// 排序后的类别列表
const sortedCategories = computed(() => {
  return [...categories.value].sort((a, b) => {
    const indexA = categoryOrder.indexOf(a.category_id);
    const indexB = categoryOrder.indexOf(b.category_id);
    return indexA - indexB;
  });
});

// Docker Registry 平台配置文档
const dockerRegistryDocs = [
  {
    platform: 'Windows (Docker Desktop)',
    steps: [
      '打开 Docker Desktop',
      '点击右上角齿轮图标 ⚙️ 进入 Settings',
      '选择左侧 "Docker Engine"',
      '在 JSON 配置中添加：\n"registry-mirrors": [\n  "https://docker.1ms.run",\n  "https://docker.m.daocloud.io"\n]',
      '点击 "Apply & restart" 重启 Docker'
    ]
  },
  {
    platform: 'macOS (Docker Desktop)',
    steps: [
      '打开 Docker Desktop',
      '点击右上角齿轮图标 ⚙️ 进入 Preferences',
      '选择左侧 "Docker Engine"',
      '在 JSON 配置中添加：\n"registry-mirrors": [\n  "https://docker.1ms.run",\n  "https://docker.m.daocloud.io"\n]',
      '点击 "Apply & restart" 重启 Docker'
    ]
  },
  {
    platform: 'Linux (systemd)',
    steps: [
      '创建或编辑配置文件：sudo mkdir -p /etc/docker',
      '编辑文件：sudo nano /etc/docker/daemon.json',
      '添加以下内容：\n{\n  "registry-mirrors": [\n    "https://docker.1ms.run",\n    "https://docker.m.daocloud.io"\n  ]\n}',
      '保存后重启 Docker：\nsudo systemctl daemon-reload\nsudo systemctl restart docker',
      '验证配置：docker info | grep -A 5 "Registry Mirrors"'
    ]
  },
  {
    platform: '推荐镜像源',
    steps: [
      '🇨🇳 壹秒镜像：https://docker.1ms.run（稳定推荐）',
      '🇨 DaoCloud：https://docker.m.daocloud.io',
      '🇨🇳 阿里云（需登录）：https://<your-id>.mirror.aliyuncs.com',
      'ℹ️ 配置多个镜像源可实现自动故障转移'
    ]
  }
];

// 展开的平台
const expandedPlatforms = ref<Set<string>>(new Set());

// 切换平台展开状态
function togglePlatform(platform: string) {
  if (expandedPlatforms.value.has(platform)) {
    expandedPlatforms.value.delete(platform);
  } else {
    expandedPlatforms.value.add(platform);
  }
}

// 编辑对话框状态
const showEditDialog = ref(false);
const editingOption = ref<MirrorSourceOption | null>(null);
const editValue = ref('');
const editDescription = ref('');
const isCustomEdit = ref(false);

// 加载镜像源列表
async function loadMirrorList() {
  loading.value = true;
  
  try {
    const data = await invoke<MergedMirrorCategory[]>('get_merged_mirror_list');
    categories.value = data;
  } catch (e) {
    showToast(`加载镜像源列表失败: ${e}`, 'error');
  } finally {
    loading.value = false;
  }
}

// 获取当前类别的选项列表
function getCurrentOptions(): MirrorSourceOption[] {
  const category = categories.value.find(c => c.category_id === selectedCategory.value);
  return category?.options || [];
}

// 获取当前类别对象
function getCurrentCategory(): MergedMirrorCategory | undefined {
  return categories.value.find(c => c.category_id === selectedCategory.value);
}

// 测试连接
async function testConnection(option: MirrorSourceOption) {
  if (!option.value) return;
  
  const optionKey = `${selectedCategory.value}-${option.id}`;
  testingOptions.value.add(optionKey);
  
  try {
    const result = await invoke<boolean>('test_mirror', { url: option.value });
    if (result) {
      showToast(`✅ ${option.name} 连接成功！`, 'success');
    } else {
      showToast(`❌ ${option.name} 连接失败，请检查网络或镜像源地址`, 'error');
    }
  } catch (e) {
    showToast(`测试失败: ${e}`, 'error');
  } finally {
    testingOptions.value.delete(optionKey);
  }
}

// 检查选项是否正在测试
function isTesting(option: MirrorSourceOption): boolean {
  const optionKey = `${selectedCategory.value}-${option.id}`;
  return testingOptions.value.has(optionKey);
}

// 选择镜像源并自动应用配置
async function selectMirror(option: MirrorSourceOption) {
  loading.value = true;
  
  try {
    // 1. 保存用户选择
    await invoke('save_selected_mirror_option', {
      categoryId: selectedCategory.value,
      optionId: option.id
    });
    
    // 2. 立即应用到 .env 文件
    await invoke('update_single_mirror', {
      category: selectedCategory.value,
      source: option.value
    });
    
    showToast(`已选择并应用 ${option.name}`, 'success');
    
    // 重新加载数据
    await loadMirrorList();
  } catch (e) {
    showToast(`保存失败: ${e}`, 'error');
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

// 保存编辑（更新已有选项）并自动应用配置
async function saveEdit() {
  if (!editValue.value.trim()) {
    showToast('请输入镜像源地址', 'warning');
    return;
  }
  
  loading.value = true;
  
  try {
    // 1. 保存用户自定义配置
    await invoke('save_user_mirror_category', {
      categoryId: selectedCategory.value,
      source: editValue.value.trim(),
      description: editDescription.value || undefined
    });
    
    // 2. 立即应用到 .env 文件
    await invoke('update_single_mirror', {
      category: selectedCategory.value,
      source: editValue.value.trim()
    });
    
    showToast('镜像源已更新并应用！', 'success');
    showEditDialog.value = false;
    
    // 重新加载数据
    await loadMirrorList();
  } catch (e) {
    showToast(`更新失败: ${e}`, 'error');
  } finally {
    loading.value = false;
  }
}

// 保存自定义镜像源并自动应用配置
async function saveCustomMirror() {
  if (!editValue.value.trim()) {
    showToast('请输入镜像源地址', 'warning');
    return;
  }
  
  loading.value = true;
  
  try {
    // 1. 保存用户自定义配置
    await invoke('save_user_mirror_category', {
      categoryId: selectedCategory.value,
      source: editValue.value.trim(),
      description: editDescription.value || undefined
    });
    
    // 2. 立即应用到 .env 文件
    await invoke('update_single_mirror', {
      category: selectedCategory.value,
      source: editValue.value.trim()
    });
    
    showToast('自定义镜像源已保存并应用！', 'success');
    showEditDialog.value = false;
    
    // 重新加载数据
    await loadMirrorList();
  } catch (e) {
    showToast(`保存失败: ${e}`, 'error');
  } finally {
    loading.value = false;
  }
}

// 删除自定义镜像源
async function removeCustomMirror() {
  const confirmed = await showConfirm({
    title: '删除确认',
    message: '确定要删除自定义镜像源吗？',
    confirmText: '删除',
    type: 'danger'
  });
  
  if (!confirmed) return;
  
  loading.value = true;
  
  try {
    // 1. 从用户配置中删除
    await invoke('remove_user_mirror_category', {
      categoryId: selectedCategory.value
    });
    
    // 2. 同步更新 .env 文件（恢复为默认值）
    let defaultValue = '';
    switch (selectedCategory.value) {
      case 'docker_registry':
        defaultValue = '';
        break;
      case 'apt':
        defaultValue = 'https://deb.debian.org/debian';
        break;
      case 'composer':
        defaultValue = 'https://packagist.org';
        break;
      case 'npm':
        defaultValue = 'https://registry.npmjs.org';
        break;
      case 'github_proxy':
        defaultValue = '';
        break;
    }
    
    await invoke('update_single_mirror', {
      category: selectedCategory.value,
      source: defaultValue
    });
    
    showToast('已恢复为默认配置并更新 .env', 'success');
    showEditDialog.value = false;
    
    // 重新加载数据
    await loadMirrorList();
  } catch (e) {
    showToast(`删除失败: ${e}`, 'error');
  } finally {
    loading.value = false;
  }
}

// 重置所有自定义
async function resetAllOverrides() {
  const confirmed = await showConfirm({
    title: '重置所有自定义',
    message: '确定要重置所有自定义镜像源配置吗？此操作不可撤销。',
    confirmText: '重置',
    type: 'danger'
  });
  
  if (!confirmed) return;
  
  loading.value = true;
  
  try {
    // 1. 重置所有用户自定义配置
    await invoke('reset_all_mirror_overrides');
    
    // 2. 同步更新 .env 文件（恢复所有类别为默认值）
    const defaults = {
      docker_registry: '',
      apt: 'https://deb.debian.org/debian',
      composer: 'https://packagist.org',
      npm: 'https://registry.npmjs.org',
      github_proxy: ''
    };
    
    for (const [category, defaultValue] of Object.entries(defaults)) {
      await invoke('update_single_mirror', {
        category,
        source: defaultValue
      });
    }
    
    showToast('已重置所有自定义配置并更新 .env', 'success');
    
    // 重新加载数据
    await loadMirrorList();
  } catch (e) {
    showToast(`重置失败: ${e}`, 'error');
  } finally {
    loading.value = false;
  }
}

// 复制镜像源地址
async function copyUrl(url: string) {
  try {
    await navigator.clipboard.writeText(url);
    showToast('已复制到剪贴板！', 'success');
  } catch (e) {
    showToast('复制失败', 'error');
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
      </div>
    </header>
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
          v-for="category in sortedCategories"
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

      <!-- Add Custom Button (仅非 Docker Registry 显示) -->
      <div v-if="selectedCategory !== 'docker_registry'" class="mb-3 flex-shrink-0">
        <button
          @click="openCustomEdit"
          class="px-4 py-2 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg text-sm transition"
        >
          ➕ 新增自定义
        </button>
      </div>

      <!-- Docker Registry: 文档引导 -->
      <div v-if="selectedCategory === 'docker_registry'" class="flex-1 overflow-auto min-h-0">
        <div class="space-y-3">
          <!-- 提示框 -->
          <div class="p-4 bg-yellow-500/10 border border-yellow-500/30 rounded-lg">
            <p class="text-yellow-400 text-sm font-medium">⚠️ Docker Registry 镜像源需要在 Docker Desktop 全局设置中配置</p>
            <p class="text-slate-400 text-xs mt-1">由于 Docker 架构限制，无法通过本项目自动配置，请按照下方指引手动配置</p>
          </div>

          <!-- 平台配置文档 -->
          <div 
            v-for="doc in dockerRegistryDocs" 
            :key="doc.platform"
            class="border border-slate-700 rounded-lg overflow-hidden"
          >
            <button
              @click="togglePlatform(doc.platform)"
              class="w-full px-4 py-3 bg-slate-800 hover:bg-slate-700 text-left flex items-center justify-between transition"
            >
              <span class="font-medium text-sm">{{ doc.platform }}</span>
              <svg 
                class="w-5 h-5 text-slate-400 transition-transform" 
                :class="{ 'rotate-180': expandedPlatforms.has(doc.platform) }"
                fill="none" 
                stroke="currentColor" 
                viewBox="0 0 24 24"
              >
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
              </svg>
            </button>
            <div 
              v-show="expandedPlatforms.has(doc.platform)"
              class="px-4 py-3 bg-slate-900 border-t border-slate-700"
            >
              <ol class="space-y-2 text-sm text-slate-300">
                <li 
                  v-for="(step, index) in doc.steps" 
                  :key="index"
                  class="flex gap-2"
                >
                  <span class="flex-shrink-0 w-5 h-5 bg-blue-600 rounded-full flex items-center justify-center text-xs text-white font-medium">
                    {{ index + 1 }}
                  </span>
                  <span class="whitespace-pre-line">{{ step }}</span>
                </li>
              </ol>
            </div>
          </div>
        </div>
      </div>

      <!-- 其他镜像源: Options Table -->
      <div v-else class="flex-1 overflow-auto min-h-0">
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
                    v-if="getCurrentCategory()?.selected_id && getCurrentCategory()?.selected_id === option.id" 
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
                        getCurrentCategory()?.selected_id && getCurrentCategory()?.selected_id === option.id
                          ? 'bg-blue-600 text-white cursor-default'
                          : 'bg-slate-700 hover:bg-slate-600 text-white'
                      ]"
                      :disabled="!!getCurrentCategory()?.selected_id && getCurrentCategory()?.selected_id === option.id"
                      title="选择此镜像源"
                    >
                      选择
                    </button>
                    <button
                      v-if="option.value"
                      @click="testConnection(option)"
                      :disabled="isTesting(option)"
                      class="px-3 py-1.5 bg-green-600 hover:bg-green-700 text-white rounded text-xs transition whitespace-nowrap disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-1.5"
                      title="测试连接"
                    >
                      <span v-if="isTesting(option)" class="inline-block animate-spin rounded-full h-3 w-3 border-b-2 border-white"></span>
                      {{ isTesting(option) ? '测试中...' : '测试' }}
                    </button>
                    <button
                      @click="copyUrl(option.value)"
                      class="px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-white rounded text-xs transition whitespace-nowrap"
                      title="复制地址"
                    >
                      复制
                    </button>
                    <button
                      v-if="option.id === 'custom' || option.value"
                      @click="openEditDialog(option)"
                      class="px-3 py-1.5 bg-yellow-600 hover:bg-yellow-700 text-white rounded text-xs transition whitespace-nowrap"
                      title="编辑"
                    >
                      编辑
                    </button>
                    <button
                      v-if="option.id === 'custom'"
                      @click="removeCustomMirror"
                      class="px-3 py-1.5 bg-rose-600 hover:bg-rose-700 text-white rounded text-xs transition whitespace-nowrap"
                      title="删除自定义"
                    >
                      删除
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
          <li>点击"选择"按钮后会自动应用配置到 .env 文件</li>
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
            @click="isCustomEdit ? saveCustomMirror() : saveEdit()"
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
