<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { VersionMappings, VersionInfo, ServiceType } from '../types/version-mapping';

const versionMappings = ref<VersionMappings | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);
const successMsg = ref<string | null>(null);
const selectedService = ref<ServiceType>('mysql');

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

onMounted(() => {
  loadVersionMappings();
});
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden">
    <header class="mb-6">
      <h1 class="text-3xl font-bold">软件设置</h1>
      <p class="text-slate-400 text-sm mt-1">管理 Docker 镜像版本映射配置</p>
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
          <thead class="sticky top-0 bg-slate-900 z-10">
            <tr class="border-b border-slate-700">
              <th class="py-3 px-4 text-slate-300 font-semibold">应用名称</th>
              <th class="py-3 px-4 text-slate-300 font-semibold">版本号</th>
              <th class="py-3 px-4 text-slate-300 font-semibold">Docker 镜像标签</th>
              <th class="py-3 px-4 text-slate-300 font-semibold">完整镜像名</th>
              <th class="py-3 px-4 text-slate-300 font-semibold">状态</th>
              <th class="py-3 px-4 text-slate-300 font-semibold">备注</th>
              <th class="py-3 px-4 text-slate-300 font-semibold">操作</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="version in getCurrentVersions()"
              :key="version.version"
              class="border-b border-slate-800 hover:bg-slate-800/50 transition"
            >
              <td class="py-3 px-4 text-slate-200">
                {{ serviceLabels[selectedService] }}
              </td>
              <td class="py-3 px-4">
                <code class="text-emerald-400 bg-slate-800 px-2 py-1 rounded text-sm">
                  {{ version.version }}
                </code>
              </td>
              <td class="py-3 px-4">
                <code class="text-blue-400 bg-slate-800 px-2 py-1 rounded text-sm">
                  {{ version.tag }}
                </code>
              </td>
              <td class="py-3 px-4">
                <code class="text-purple-400 bg-slate-800 px-2 py-1 rounded text-xs">
                  {{ version.full_name }}
                </code>
              </td>
              <td class="py-3 px-4">
                <span
                  :class="[
                    'inline-flex items-center px-2 py-1 rounded-full text-xs font-medium',
                    version.eol
                      ? 'bg-rose-500/20 text-rose-400'
                      : 'bg-green-500/20 text-green-400'
                  ]"
                >
                  {{ version.eol ? '⚠️ EOL' : '✅ 活跃' }}
                </span>
              </td>
              <td class="py-3 px-4 text-slate-400 text-sm">
                {{ version.description || '-' }}
              </td>
              <td class="py-3 px-4">
                <button
                  @click="copyImageName(version.full_name)"
                  class="px-3 py-1 bg-slate-700 hover:bg-slate-600 rounded text-xs font-medium transition flex items-center gap-1"
                >
                  <svg xmlns="http://www.w3.org/2000/svg" class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                  </svg>
                  复制
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- Footer Info -->
      <div class="mt-6 p-4 bg-blue-500/10 border border-blue-500/20 rounded-xl">
        <div class="flex items-start gap-3">
          <svg xmlns="http://www.w3.org/2000/svg" class="w-5 h-5 text-blue-400 flex-shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <div class="text-sm text-slate-300">
            <p class="font-semibold text-blue-300 mb-1">💡 提示</p>
            <ul class="list-disc list-inside space-y-1 text-slate-400">
              <li>EOL (End of Life) 表示该版本已停止维护，建议使用更新版本</li>
              <li>点击"复制"按钮可快速获取完整的 Docker 镜像名称</li>
              <li>如需自定义镜像标签，请联系开发者修改配置文件</li>
            </ul>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
