<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';

interface Container {
  id: String;
  name: String;
  image: String;
  status: String;
  state: String;
  ports: number[];
}

const containers = ref<Container[]>([]);
const loading = ref(false);
const logs = ref<string[]>([]);

const addLog = (msg: string) => {
  const time = new Date().toLocaleTimeString();
  logs.value.unshift(`[${time}] ${msg}`);
  if (logs.value.length > 50) logs.value.pop();
};

const refreshContainers = async () => {
  try {
    loading.value = true;
    containers.value = await invoke('list_containers');
    addLog('已刷新容器列表');
  } catch (e) {
    addLog(`错误: ${e}`);
  } finally {
    loading.value = false;
  }
};

const startService = async (name: String) => {
  try {
    addLog(`正在启动服务: ${name}...`);
    await invoke('start_container', { name });
    addLog(`服务 ${name} 已启动`);
    await refreshContainers();
  } catch (e) {
    addLog(`启动失败: ${e}`);
  }
};

const stopService = async (name: String) => {
  try {
    addLog(`正在停止服务: ${name}...`);
    await invoke('stop_container', { name });
    addLog(`服务 ${name} 已停止`);
    await refreshContainers();
  } catch (e) {
    addLog(`停止失败: ${e}`);
  }
};

onMounted(() => {
  refreshContainers();
  // 每 5 秒自动刷新一次
  setInterval(refreshContainers, 5000);
});
</script>

<template>
  <div class="flex h-screen w-screen overflow-hidden bg-slate-950 text-slate-200">
    <!-- Sidebar -->
    <div class="w-64 bg-slate-900 p-6 flex flex-col gap-4 border-r border-slate-800">
      <div class="text-2xl font-bold text-blue-400 mb-6 flex items-center gap-2">
        <span class="bg-blue-500 text-white p-1 rounded">PS</span> PHP-Stack
      </div>
      <div class="sidebar-item active">环境管理</div>
      <div class="sidebar-item">虚拟主机 (Vhosts)</div>
      <div class="sidebar-item">软件管理 (Docker)</div>
      <div class="sidebar-item">镜像源设置</div>
      <div class="sidebar-item">备份与恢复</div>
      <div class="mt-auto pt-4 border-t border-slate-800 text-sm text-slate-500 text-center">
        v1.0.0-beta
      </div>
    </div>

    <!-- Main Content -->
    <div class="flex-1 flex flex-col overflow-hidden p-8">
      <header class="flex justify-between items-center mb-8">
        <h1 class="text-3xl font-bold">运行状态</h1>
        <div class="flex gap-4">
          <button 
            @click="refreshContainers" 
            :disabled="loading"
            class="bg-blue-600 hover:bg-blue-700 disabled:opacity-50 px-4 py-2 rounded-lg font-medium transition"
          >
            {{ loading ? '刷新中...' : '手动刷新' }}
          </button>
        </div>
      </header>

      <!-- Container Grid -->
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6 overflow-y-auto mb-8 pr-2">
        <div v-for="c in containers" :key="c.id" class="bg-slate-900 border border-slate-800 rounded-xl p-5 hover:border-blue-500/50 transition-colors shadow-lg">
          <div class="flex justify-between items-start mb-4">
            <span class="text-slate-400 text-xs font-mono uppercase tracking-wider">{{ c.image.split(':')[0] }}</span>
            <span 
              :class="c.state.includes('Running') ? 'text-emerald-400' : 'text-rose-400'"
              class="flex items-center gap-1.5 text-xs font-bold uppercase tracking-tighter"
            >
              <span :class="c.state.includes('Running') ? 'bg-emerald-500' : 'bg-rose-500'" class="w-2 h-2 rounded-full animate-pulse"></span>
              {{ c.state.includes('Running') ? 'Running' : 'Stopped' }}
            </span>
          </div>
          <div class="text-xl font-bold mb-1 truncate" :title="c.name">{{ c.name }}</div>
          <div class="text-slate-500 text-xs mb-4">
            <span v-if="c.ports.length > 0">Ports: {{ c.ports.join(', ') }}</span>
            <span v-else>No public ports</span>
          </div>
          
          <div class="flex gap-2">
            <button 
              v-if="!c.state.includes('Running')"
              @click="startService(c.name)"
              class="flex-1 py-2 bg-emerald-600/20 hover:bg-emerald-600 text-emerald-400 hover:text-white border border-emerald-600/30 rounded text-sm font-medium transition-all"
            >
              启动
            </button>
            <button 
              v-else
              @click="stopService(c.name)"
              class="flex-1 py-2 bg-rose-600/20 hover:bg-rose-600 text-rose-400 hover:text-white border border-rose-600/30 rounded text-sm font-medium transition-all"
            >
              停止
            </button>
            <button class="px-3 py-2 bg-slate-800 hover:bg-slate-700 rounded text-sm transition border border-slate-700">
              配置
            </button>
          </div>
        </div>

        <!-- Empty State -->
        <div v-if="containers.length === 0 && !loading" class="col-span-full py-20 text-center bg-slate-900/50 border-2 border-dashed border-slate-800 rounded-2xl">
          <div class="text-slate-500 mb-2">未发现 ps- 前缀的容器</div>
          <div class="text-slate-600 text-sm">请先在“软件管理”中安装 PHP、Nginx 等环境</div>
        </div>
      </div>

      <!-- Log Panel -->
      <div class="mt-auto">
        <h2 class="text-lg font-bold mb-3 flex items-center gap-2 text-slate-400">
          <span class="w-2 h-2 bg-blue-500 rounded-full"></span> 实时日志
        </h2>
        <div class="bg-black/40 backdrop-blur-sm p-4 rounded-xl font-mono text-sm text-blue-300/80 border border-slate-800 h-40 overflow-y-auto scrollbar-hide shadow-inner">
          <div v-for="(log, i) in logs" :key="i" class="mb-1 last:mb-0">
            {{ log }}
          </div>
          <div v-if="logs.length === 0" class="text-slate-600 italic">等待操作中...</div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.sidebar-item {
  @apply px-4 py-3 rounded-lg transition-all cursor-pointer text-slate-400 hover:bg-slate-800 hover:text-slate-100;
}
.sidebar-item.active {
  @apply bg-blue-600/10 text-blue-400 border border-blue-600/20;
}
.scrollbar-hide::-webkit-scrollbar {
  display: none;
}
</style>
