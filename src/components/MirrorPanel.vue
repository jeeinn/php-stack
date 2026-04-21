<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';

interface MirrorCategory {
  key: string;
  label: string;
  icon: string;
  value: string;
  customValue: string; // 自定义值
  isCustom: boolean; // 是否使用自定义
  testing: boolean;
  testResult: 'idle' | 'success' | 'fail';
}

const applying = ref(false);
const error = ref<string | null>(null);
const successMsg = ref<string | null>(null);
const showInfo = ref(false); // 提示信息默认折叠

const categories = ref<MirrorCategory[]>([
  { 
    key: 'docker_registry', 
    label: 'Docker Registry', 
    icon: '🐳', 
    value: '', 
    customValue: '',
    isCustom: false,
    testing: false, 
    testResult: 'idle' 
  },
  { 
    key: 'apt', 
    label: 'APT / Debian', 
    icon: '📦', 
    value: '', 
    customValue: '',
    isCustom: false,
    testing: false, 
    testResult: 'idle' 
  },
  { 
    key: 'composer', 
    label: 'Composer', 
    icon: '🎵', 
    value: '', 
    customValue: '',
    isCustom: false,
    testing: false, 
    testResult: 'idle' 
  },
  { 
    key: 'npm', 
    label: 'NPM', 
    icon: '📗', 
    value: '', 
    customValue: '',
    isCustom: false,
    testing: false, 
    testResult: 'idle' 
  },
  { 
    key: 'github', 
    label: 'GitHub Proxy', 
    icon: '🐙', 
    value: '', 
    customValue: '',
    isCustom: false,
    testing: false, 
    testResult: 'idle' 
  },
]);

// 镜像源选项（不包含“官方默认”，因为空值即代表官方）
const mirrorOptions: Record<string, { label: string; url: string }[]> = {
  docker_registry: [
    { label: '阿里云', url: 'https://registry.cn-hangzhou.aliyuncs.com' },
    { label: '腾讯云', url: 'https://mirror.ccs.tencentyun.com' },
    { label: '中科大', url: 'https://docker.mirrors.ustc.edu.cn' },
    { label: '清华大学', url: 'https://docker.mirrors.tuna.tsinghua.edu.cn' },
  ],
  apt: [
    { label: '阿里云', url: 'https://mirrors.aliyun.com/debian' },
    { label: '清华大学', url: 'https://mirrors.tuna.tsinghua.edu.cn/debian' },
    { label: '腾讯云', url: 'https://mirrors.cloud.tencent.com/debian' },
    { label: '中科大', url: 'https://mirrors.ustc.edu.cn/debian' },
  ],
  composer: [
    { label: '阿里云', url: 'https://mirrors.aliyun.com/composer/' },
    { label: '腾讯云', url: 'https://mirrors.cloud.tencent.com/composer/' },
    { label: '华为云', url: 'https://repo.huaweicloud.com/repository/php/' },
  ],
  npm: [
    { label: '淘宝 (npmmirror)', url: 'https://registry.npmmirror.com' },
    { label: '腾讯云', url: 'https://mirrors.cloud.tencent.com/npm/' },
    { label: '华为云', url: 'https://repo.huaweicloud.com/repository/npm/' },
  ],
  github: [
    { label: 'ghproxy.com', url: 'https://ghproxy.com' },
    { label: 'github.moeyy.xyz', url: 'https://github.moeyy.xyz' },
    { label: 'mirror.ghproxy.com', url: 'https://mirror.ghproxy.com' },
  ],
};

async function loadStatus() {
  try {
    const status = await invoke<Record<string, string>>('get_mirror_status');
    for (const cat of categories.value) {
      const value = status[cat.key];
      if (value && value !== 'default' && value !== '') {
        // 检查是否是预定义选项
        const options = mirrorOptions[cat.key] || [];
        const isPreset = options.some(opt => opt.url === value);
        
        if (isPreset) {
          cat.value = value;
          cat.isCustom = false;
        } else {
          // 自定义镜像源
          cat.value = '__custom__';
          cat.customValue = value;
          cat.isCustom = true;
        }
      } else {
        // 官方默认（空值）
        cat.value = '';
        cat.isCustom = false;
      }
    }
  } catch (e) {
    console.error('加载镜像源状态失败:', e);
  }
}

// 处理下拉框变化
function handleCategoryChange(cat: MirrorCategory) {
  if (cat.value === '__custom__') {
    cat.isCustom = true;
    // 如果之前有自定义值，保留；否则清空
    if (!cat.customValue) {
      cat.customValue = '';
    }
  } else {
    cat.isCustom = false;
    cat.customValue = ''; // 清空自定义值
  }
}

async function testConnection(cat: MirrorCategory) {
  const urlToTest = cat.isCustom ? cat.customValue : cat.value;
  if (!urlToTest) return;
  
  cat.testing = true;
  cat.testResult = 'idle';
  try {
    const result = await invoke<boolean>('test_mirror', { url: urlToTest });
    cat.testResult = result ? 'success' : 'fail';
  } catch {
    cat.testResult = 'fail';
  } finally {
    cat.testing = false;
  }
}

async function handleApply() {
  applying.value = true;
  error.value = null;
  successMsg.value = null;
  try {
    for (const cat of categories.value) {
      // 确定要保存的值
      let valueToSave = '';
      if (cat.isCustom) {
        valueToSave = cat.customValue.trim();
      } else if (cat.value && cat.value !== '__custom__') {
        valueToSave = cat.value;
      }
      // 空值表示使用官方默认
      
      if (valueToSave || cat.value === '') {
        await invoke('update_single_mirror', { category: cat.key, source: valueToSave });
      }
    }
    successMsg.value = '镜像源配置已应用';
    setTimeout(() => { successMsg.value = null; }, 3000);
  } catch (e) {
    error.value = e as string;
  } finally {
    applying.value = false;
  }
}

// 打开 Docker Desktop 设置（复制链接到剪贴板）
async function openDockerSettings() {
  const dockerConfig = `{
  "registry-mirrors": [
    "https://registry.cn-hangzhou.aliyuncs.com"
  ]
}`;
  
  try {
    await navigator.clipboard.writeText(dockerConfig);
    successMsg.value = 'Docker 配置已复制到剪贴板！请在 Docker Desktop → Settings → Docker Engine 中粘贴';
    setTimeout(() => { successMsg.value = null; }, 5000);
  } catch (e) {
    console.error('复制失败:', e);
    error.value = '复制失败，请手动配置 Docker Registry Mirror';
  }
}

onMounted(() => {
  loadStatus();
});
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden">
    <header class="flex justify-between items-center mb-6">
      <div>
        <h1 class="text-3xl font-bold">镜像源管理</h1>
        <p class="text-slate-400 text-sm mt-1">统一管理 Docker、APT、Composer、NPM 镜像源</p>
      </div>
      <button
        @click="handleApply"
        :disabled="applying"
        class="px-5 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg font-medium transition disabled:opacity-50"
      >
        {{ applying ? '应用中...' : '应用配置' }}
      </button>
    </header>

    <!-- Messages -->
    <div v-if="error" class="mb-4 p-4 bg-rose-500/10 border border-rose-500/20 rounded-xl text-rose-400 text-sm">
      {{ error }}
    </div>
    <div v-if="successMsg" class="mb-4 p-4 bg-emerald-500/10 border border-emerald-500/20 rounded-xl text-emerald-400 text-sm">
      {{ successMsg }}
    </div>

    <div class="flex-1 overflow-y-auto pr-2 space-y-6">
      <!-- 使用说明 (可折叠) -->
      <section class="bg-blue-500/10 border border-blue-500/20 rounded-xl overflow-hidden">
        <button
          @click="showInfo = !showInfo"
          class="w-full px-6 py-4 flex items-center justify-between hover:bg-blue-500/5 transition"
        >
          <h2 class="text-base font-bold flex items-center gap-2 text-blue-400">
            <svg xmlns="http://www.w3.org/2000/svg" class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            镜像源说明
          </h2>
          <svg 
            xmlns="http://www.w3.org/2000/svg" 
            class="w-5 h-5 text-slate-400 transition-transform duration-200"
            :class="showInfo ? 'rotate-180' : ''"
            fill="none" 
            viewBox="0 0 24 24" 
            stroke="currentColor" 
            stroke-width="2"
          >
            <path stroke-linecap="round" stroke-linejoin="round" d="m19 9-7 7-7-7" />
          </svg>
        </button>
              
        <transition name="slide-down">
          <div v-show="showInfo" class="px-6 pb-5 border-t border-blue-500/10">
            <div class="pt-4 space-y-3 text-sm text-slate-300">
              <div>
                <p class="font-semibold text-blue-300 mb-1">📌 作用：</p>
                <p class="text-slate-400 text-xs">加速 Docker 镜像拉取、APT/Composer/NPM 包安装、GitHub 资源下载</p>
              </div>
                    
              <div>
                <p class="font-semibold text-blue-300 mb-1">💡 提示：</p>
                <ul class="list-disc list-inside space-y-1 ml-2 text-xs text-slate-400">
                  <li>国内用户建议选阿里云/腾讯云</li>
                  <li>修改后需<strong class="text-yellow-400">重新构建</strong>镜像生效</li>
                  <li>点击“测试连接”验证可用性</li>
                </ul>
              </div>
                    
              <div class="bg-slate-800/50 rounded-lg p-3 border border-slate-700">
                <p class="font-semibold text-amber-300 mb-1 text-xs flex items-center gap-1">
                  ⚠️ Docker Registry 需全局配置
                </p>
                <button
                  @click="openDockerSettings"
                  class="text-xs px-3 py-1.5 bg-blue-600 hover:bg-blue-700 rounded transition text-white mt-2"
                >
                  📋 复制 Docker 配置
                </button>
              </div>
            </div>
          </div>
        </transition>
      </section>

      <!-- Individual Categories -->
      <section
        v-for="cat in categories"
        :key="cat.key"
        class="bg-slate-900 border border-slate-800 rounded-xl p-6"
      >
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-lg font-bold">{{ cat.icon }} {{ cat.label }}</h2>
          <div class="flex items-center gap-2">
            <span
              v-if="cat.testResult === 'success'"
              class="text-xs text-emerald-400 flex items-center gap-1"
            >
              <span class="w-2 h-2 bg-emerald-500 rounded-full"></span> 连接成功
            </span>
            <span
              v-if="cat.testResult === 'fail'"
              class="text-xs text-amber-400 flex items-center gap-1"
            >
              <span class="w-2 h-2 bg-amber-500 rounded-full"></span> 连接失败
            </span>
            <button
              @click="testConnection(cat)"
              :disabled="cat.testing || !cat.value"
              class="text-xs px-3 py-1.5 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg transition disabled:opacity-50"
            >
              {{ cat.testing ? '测试中...' : '测试连接' }}
            </button>
          </div>
        </div>
        <select
          v-model="cat.value"
          @change="handleCategoryChange(cat)"
          class="w-full bg-slate-800 border border-slate-700 rounded-lg px-4 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500 mb-3"
        >
          <option value="">🌐 官方默认（不加速）</option>
          <option
            v-for="opt in mirrorOptions[cat.key]"
            :key="opt.url"
            :value="opt.url"
          >
            {{ opt.label }}
          </option>
          <option value="__custom__">✏️ 自定义...</option>
        </select>
        
        <!-- 自定义输入框 -->
        <div v-if="cat.isCustom" class="mt-2">
          <input
            v-model="cat.customValue"
            type="text"
            placeholder="请输入镜像源地址，如: https://example.com"
            class="w-full bg-slate-800 border border-slate-700 rounded-lg px-4 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500"
          />
          <p class="text-xs text-slate-500 mt-1">💡 提示：留空将使用官方默认源</p>
        </div>
        <div v-if="cat.testResult === 'fail'" class="mt-2 text-xs text-amber-400">
          连接测试失败，但不影响保存。建议选择其他可用源。
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
@reference "tailwindcss";

.slide-down-enter-active,
.slide-down-leave-active {
  transition: all 0.3s ease;
  max-height: 500px;
  opacity: 1;
}

.slide-down-enter-from,
.slide-down-leave-to {
  max-height: 0;
  opacity: 0;
  padding-top: 0;
  padding-bottom: 0;
  overflow: hidden;
}
</style>
