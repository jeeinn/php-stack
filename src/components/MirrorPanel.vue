<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke } from '@tauri-apps/api/core';
import type { MergedMirrorCategory, MirrorSourceOption } from '../types/env-config';
import { showToast } from '../composables/useToast';
import { showConfirm } from '../composables/useConfirmDialog';

const { t } = useI18n();

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

// 从 i18n 获取 tm 函数（用于获取数组类型的翻译）
const { tm } = useI18n();

// Docker Registry 平台配置文档（从 i18n 加载，URL 动态生成）
const dockerRegistryDocs = computed(() => {
  // 从已加载的 categories 中提取 docker_registry 的所有镜像源
  const dockerCategory = categories.value.find(c => c.category_id === 'docker_registry');
  const mirrors = dockerCategory?.options || [];
  
  // 提取所有非空的 URL（过滤掉官方默认的 empty value）
  const mirrorUrls = mirrors
    .map(m => m.value)
    .filter(Boolean); // 过滤空字符串
  
  // 格式化 URLs 为 JSON 数组格式
  const formattedUrls = mirrorUrls.map(url => `"${url}"`).join(',\n  ');
  
  // 如果没有可用的镜像源，使用占位符提示
  const urlsText = formattedUrls || '"https://your-mirror-url"';
  
  // 获取翻译数组
  // 注意：vue-i18n v10 中，t() 函数对数组类型的支持有限
  // 使用 tm() 方法可以直接获取翻译消息（Message）
  const windowsSteps = tm('mirror.dockerRegistry.steps.windows') as string[] || [];
  const macosSteps = tm('mirror.dockerRegistry.steps.macos') as string[] || [];
  const linuxSteps = tm('mirror.dockerRegistry.steps.linux') as string[] || [];
  
  // 替换 {urls} 占位符
  const replaceUrls = (step: string) => step.replace('{urls}', urlsText);
  
  const result = [
    {
      platform: t('mirror.dockerRegistry.platforms.windows'),
      steps: windowsSteps.map(replaceUrls)
    },
    {
      platform: t('mirror.dockerRegistry.platforms.macos'),
      steps: macosSteps.map(replaceUrls)
    },
    {
      platform: t('mirror.dockerRegistry.platforms.linux'),
      steps: linuxSteps.map(replaceUrls)
    },
    {
      platform: t('mirror.dockerRegistry.platforms.recommended'),
      steps: mirrors.map(m => 
        m.value ? `${m.name}：${m.value}` : `${m.name}（使用官方源）`
      )
    }
  ];
  
  return result;
});

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
    showToast(t('mirror.toast.loadFailed', { error: e }), 'error');
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
      showToast(t('mirror.toast.testSuccess', { name: option.name }), 'success');
    } else {
      showToast(t('mirror.toast.testFailed', { name: option.name }), 'error');
    }
  } catch (e) {
    showToast(t('mirror.toast.testError', { error: e }), 'error');
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
    
    showToast(t('mirror.toast.selected', { name: option.name }), 'success');
    
    // 重新加载数据
    await loadMirrorList();
  } catch (e) {
    showToast(t('mirror.toast.saveFailed', { error: e }), 'error');
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
    showToast(t('mirror.toast.urlRequired'), 'warning');
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
    
    showToast(t('mirror.toast.updated'), 'success');
    showEditDialog.value = false;
    
    // 重新加载数据
    await loadMirrorList();
  } catch (e) {
    showToast(t('mirror.toast.updateFailed', { error: e }), 'error');
  } finally {
    loading.value = false;
  }
}

// 保存自定义镜像源并自动应用配置
async function saveCustomMirror() {
  if (!editValue.value.trim()) {
    showToast(t('mirror.toast.urlRequired'), 'warning');
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
    
    showToast(t('mirror.toast.customSaved'), 'success');
    showEditDialog.value = false;
    
    // 重新加载数据
    await loadMirrorList();
  } catch (e) {
    showToast(t('mirror.toast.saveFailed', { error: e }), 'error');
  } finally {
    loading.value = false;
  }
}

// 删除自定义镜像源
async function removeCustomMirror() {
  const confirmed = await showConfirm({
    title: t('mirror.confirm.deleteTitle'),
    message: t('mirror.confirm.deleteMessage'),
    confirmText: t('common.delete'),
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
    
    showToast(t('mirror.toast.deleted'), 'success');
    showEditDialog.value = false;
    
    // 重新加载数据
    await loadMirrorList();
  } catch (e) {
    showToast(t('mirror.toast.deleteFailed', { error: e }), 'error');
  } finally {
    loading.value = false;
  }
}

// 重置所有自定义
async function resetAllOverrides() {
  const confirmed = await showConfirm({
    title: t('mirror.confirm.resetTitle'),
    message: t('mirror.confirm.resetMessage'),
    confirmText: t('common.reset'),
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
    
    showToast(t('mirror.toast.resetDone'), 'success');
    
    // 重新加载数据
    await loadMirrorList();
  } catch (e) {
    showToast(t('mirror.toast.resetFailed', { error: e }), 'error');
  } finally {
    loading.value = false;
  }
}

// 复制镜像源地址
async function copyUrl(url: string) {
  try {
    await navigator.clipboard.writeText(url);
    showToast(t('mirror.toast.copied'), 'success');
  } catch (e) {
    showToast(t('mirror.toast.copyFailed'), 'error');
  }
}

onMounted(() => {
  loadMirrorList();
});
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-200 transition-colors duration-300">
    <header class="mb-4 sm:mb-6 flex flex-col sm:flex-row justify-between items-start gap-3">
      <div>
        <p class="text-slate-500 dark:text-slate-400 text-xs sm:text-sm">{{ $t('mirror.subtitle') }}</p>
      </div>
      <div class="flex gap-2 w-full sm:w-auto">
        <button
          @click="resetAllOverrides"
          class="w-full sm:w-auto px-4 py-2 bg-rose-600 hover:bg-rose-700 text-white rounded-lg transition text-sm"
        >
          {{ $t('mirror.resetAll') }}
        </button>
      </div>
    </header>
    <div v-if="loading && categories.length === 0" class="flex-1 flex items-center justify-center">
      <div class="text-center">
        <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-500 mx-auto mb-4"></div>
        <p class="text-slate-500 dark:text-slate-400">{{ $t('common.loading') }}</p>
      </div>
    </div>

    <!-- Content -->
    <div v-else-if="categories.length > 0" class="flex-1 flex flex-col min-h-0">
      <!-- Category Tabs -->
      <div class="flex gap-2 mb-3 sm:mb-4 border-b border-slate-200 dark:border-slate-700 pb-2 flex-shrink-0 overflow-x-auto scrollbar-hide">
        <button
          v-for="category in sortedCategories"
          :key="category.category_id"
          @click="selectedCategory = category.category_id"
          :class="[
            'px-3 sm:px-4 py-1.5 rounded-lg font-medium transition whitespace-nowrap text-xs sm:text-sm',
            selectedCategory === category.category_id
              ? 'bg-blue-600 text-white'
              : 'bg-white dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700'
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
          class="w-full sm:w-auto px-4 py-2 bg-white dark:bg-slate-800 hover:bg-slate-100 dark:hover:bg-slate-700 border border-slate-300 dark:border-slate-700 text-slate-700 dark:text-slate-300 rounded-lg text-sm transition"
        >
          {{ $t('mirror.addCustom') }}
        </button>
      </div>

      <!-- Docker Registry: 文档引导 -->
      <div v-if="selectedCategory === 'docker_registry'" class="flex-1 overflow-auto min-h-0">
        <div class="space-y-3">
          <!-- 提示框 -->
          <div class="p-4 bg-yellow-500/10 border border-yellow-500/30 rounded-lg">
            <p class="text-yellow-600 dark:text-yellow-400 text-sm font-medium">{{ $t('mirror.dockerRegistry.warning') }}</p>
            <p class="text-slate-600 dark:text-slate-400 text-xs mt-1">{{ $t('mirror.dockerRegistry.hint') }}</p>
          </div>

          <!-- 平台配置文档 -->
          <div 
            v-for="doc in dockerRegistryDocs" 
            :key="doc.platform"
            class="border border-slate-200 dark:border-slate-700 rounded-lg overflow-hidden"
          >
            <button
              @click="togglePlatform(doc.platform)"
              class="w-full px-4 py-3 bg-white dark:bg-slate-800 hover:bg-slate-100 dark:hover:bg-slate-700 text-slate-900 dark:text-slate-200 text-left flex items-center justify-between transition"
            >
              <span class="font-medium text-sm">{{ doc.platform }}</span>
              <svg 
                class="w-5 h-5 text-slate-500 dark:text-slate-400 transition-transform" 
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
              class="px-4 py-3 bg-slate-50 dark:bg-slate-900 border-t border-slate-200 dark:border-slate-700"
            >
              <ol class="space-y-2 text-sm text-slate-700 dark:text-slate-300">
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
        <div class="overflow-x-auto -mx-3 sm:mx-0">
          <table class="w-full text-left border-collapse min-w-[800px]">
            <thead class="sticky top-0 bg-white dark:bg-slate-900 z-10">
              <tr class="border-b border-slate-200 dark:border-slate-700">
                <th class="py-3 px-3 text-slate-600 dark:text-slate-400 font-medium whitespace-nowrap text-sm min-w-[120px]">{{ $t('mirror.table.name') }}</th>
                <th class="py-3 px-3 text-slate-600 dark:text-slate-400 font-medium whitespace-nowrap text-sm min-w-[250px]">{{ $t('mirror.table.url') }}</th>
                <th class="py-3 px-3 text-slate-600 dark:text-slate-400 font-medium whitespace-nowrap text-sm min-w-[200px]">{{ $t('mirror.table.description') }}</th>
                <th class="py-3 px-3 text-slate-600 dark:text-slate-400 font-medium whitespace-nowrap text-sm min-w-[80px]">{{ $t('mirror.table.status') }}</th>
                <th class="py-3 px-3 text-slate-600 dark:text-slate-400 font-medium whitespace-nowrap text-sm sticky right-0 bg-white dark:bg-slate-900 z-20 w-auto">{{ $t('mirror.table.actions') }}</th>
              </tr>
            </thead>
            <tbody>
              <tr 
                v-for="option in getCurrentOptions()" 
                :key="option.id"
                :class="[
                  'border-b border-slate-200 dark:border-slate-800 hover:bg-slate-50 dark:hover:bg-slate-800/50 transition',
                  categories.find(c => c.category_id === selectedCategory)?.selected_id === option.id 
                    ? 'bg-blue-600/10 dark:bg-blue-600/10' 
                    : ''
                ]"
              >
                <td class="py-3 px-3 text-sm">
                  <span class="font-medium">{{ option.name }}</span>
                  <span 
                    v-if="getCurrentCategory()?.selected_id && getCurrentCategory()?.selected_id === option.id" 
                    class="ml-2 text-xs text-blue-400"
                  >
                    {{ $t('mirror.status.current') }}
                  </span>
                  <span 
                    v-if="option.id === 'custom'" 
                    class="ml-2 text-xs text-yellow-400"
                  >
                    {{ $t('mirror.status.custom') }}
                  </span>
                </td>
                <td class="py-3 px-3">
                  <code 
                    @click="copyUrl(option.value)"
                    class="bg-slate-100 dark:bg-slate-800 px-2 py-1.5 rounded text-xs font-mono block cursor-pointer hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-300 transition truncate"
                    :title="option.value || ''"
                  >
                    {{ option.value || '-' }}
                  </code>
                </td>
                <td class="py-3 px-3 text-slate-600 dark:text-slate-400 text-sm truncate" :title="option.description || ''">
                  {{ option.description || '-' }}
                </td>
                <td class="py-3 px-3">
                  <span 
                    v-if="option.value"
                    class="px-2 py-1 rounded text-xs font-medium bg-blue-500/20 text-blue-400 whitespace-nowrap"
                  >
                    {{ $t('mirror.status.available') }}
                  </span>
                  <span 
                    v-else
                    class="px-2 py-1 rounded text-xs font-medium bg-slate-500/20 text-slate-400 whitespace-nowrap"
                  >
                    {{ $t('mirror.status.default') }}
                  </span>
                </td>
                <td class="py-3 px-3 sticky right-0 bg-white dark:bg-slate-900 z-10 whitespace-nowrap">
                  <div class="flex items-center gap-2">
                    <button
                      @click="selectMirror(option)"
                      :class="[
                        'px-3 py-1.5 rounded text-xs transition',
                        getCurrentCategory()?.selected_id && getCurrentCategory()?.selected_id === option.id
                          ? 'bg-blue-600 text-white cursor-default'
                          : 'bg-slate-200 dark:bg-slate-700 hover:bg-slate-300 dark:hover:bg-slate-600 text-slate-700 dark:text-white'
                      ]"
                      :disabled="!!getCurrentCategory()?.selected_id && getCurrentCategory()?.selected_id === option.id"
                      title="选择此镜像源"
                    >
                      {{ $t('mirror.actions.select') }}
                    </button>
                    <button
                      v-if="option.value"
                      @click="testConnection(option)"
                      :disabled="isTesting(option)"
                      class="px-3 py-1.5 bg-green-600 hover:bg-green-700 text-white rounded text-xs transition disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-1.5"
                      title="测试连接"
                    >
                      <span v-if="isTesting(option)" class="inline-block animate-spin rounded-full h-3 w-3 border-b-2 border-white"></span>
                      <span>{{ isTesting(option) ? $t('mirror.actions.testing') : $t('mirror.actions.test') }}</span>
                    </button>
                    <button
                      v-if="option.id === 'custom' || option.value"
                      @click="openEditDialog(option)"
                      class="px-3 py-1.5 bg-yellow-600 hover:bg-yellow-700 text-white rounded text-xs transition"
                      title="编辑"
                    >
                      {{ $t('mirror.actions.edit') }}
                    </button>
                    <button
                      v-if="option.id === 'custom'"
                      @click="removeCustomMirror"
                      class="px-3 py-1.5 bg-rose-600 hover:bg-rose-700 text-white rounded text-xs transition"
                      title="删除自定义"
                    >
                      {{ $t('mirror.actions.delete') }}
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
        <p>{{ $t('mirror.hints.title') }}</p>
        <ul class="list-disc list-inside mt-2 space-y-1">
          <li>{{ $t('mirror.hints.autoApply') }}</li>
          <li>{{ $t('mirror.hints.customSaved') }}</li>
          <li>{{ $t('mirror.hints.testConnection') }}</li>
        </ul>
      </div>
    </div>

    <!-- Edit/Custom Dialog -->
    <div v-if="showEditDialog" class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50">
      <div class="bg-white dark:bg-slate-900 rounded-xl p-6 max-w-md w-full mx-4 border border-slate-200 dark:border-slate-700 shadow-2xl">
        <h2 class="text-xl font-bold mb-4 text-slate-900 dark:text-slate-200">
          {{ isCustomEdit ? $t('mirror.editDialog.titleNew') : $t('mirror.editDialog.titleEdit') }}
        </h2>
        
        <div class="space-y-4">
          <div>
            <label class="block text-sm text-slate-600 dark:text-slate-400 mb-2">
              {{ $t('mirror.editDialog.urlLabel') }} <span class="text-rose-500 dark:text-rose-400">{{ $t('mirror.editDialog.urlRequired') }}</span>
            </label>
            <input
              v-model="editValue"
              :placeholder="$t('mirror.editDialog.urlPlaceholder')"
              class="w-full px-3 py-2 bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:outline-none"
            />
          </div>
          
          <div>
            <label class="block text-sm text-slate-600 dark:text-slate-400 mb-2">{{ $t('mirror.editDialog.descLabel') }}</label>
            <textarea
              v-model="editDescription"
              :placeholder="$t('mirror.editDialog.descPlaceholder')"
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
            v-if="!isCustomEdit && editingOption?.id === 'custom'"
            @click="removeCustomMirror"
            class="px-4 py-2 bg-rose-600 hover:bg-rose-700 text-white rounded-lg transition"
          >
            {{ $t('common.delete') }}
          </button>
          <button
            @click="isCustomEdit ? saveCustomMirror() : saveEdit()"
            :disabled="!editValue.trim()"
            class="flex-1 px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-slate-600 disabled:cursor-not-allowed text-white rounded-lg transition"
          >
            {{ isCustomEdit ? $t('common.save') : $t('common.update') }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
