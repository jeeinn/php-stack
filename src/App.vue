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
const dockerError = ref<string | null>(null);
const activeTab = ref('dashboard');

const addLog = (msg: string) => {
  const time = new Date().toLocaleTimeString();
  logs.value.unshift(`[${time}] ${msg}`);
  if (logs.value.length > 50) logs.value.pop();
};

const checkDocker = async () => {
  try {
    await invoke('check_docker');
    dockerError.value = null;
    return true;
  } catch (e) {
    dockerError.value = e as string;
    addLog(`Docker 检查失败: ${e}`);
    return false;
  }
};

const refreshContainers = async () => {
  if (!(await checkDocker())) {
    containers.value = [];
    return;
  }
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

// 镜像源设置
const dockerMirrorUrl = ref('https://registry.docker-cn.com');
const updateMirror = async () => {
  try {
    addLog(`正在设置 Docker 镜像源: ${dockerMirrorUrl.value}...`);
    await invoke('set_docker_mirror', { url: dockerMirrorUrl.value });
    addLog('镜像源设置成功，请重启 Docker Desktop 生效');
  } catch (e) {
    addLog(`设置失败: ${e}`);
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
      <div 
        @click="activeTab = 'dashboard'"
        :class="{ 'active': activeTab === 'dashboard' }" 
        class="sidebar-item"
      >环境管理</div>
      <div 
        @click="activeTab = 'vhosts'"
        :class="{ 'active': activeTab === 'vhosts' }" 
        class="sidebar-item"
      >虚拟主机 (Vhosts)</div>
      <div 
        @click="activeTab = 'software'"
        :class="{ 'active': activeTab === 'software' }" 
        class="sidebar-item"
      >软件管理 (Docker)</div>
      <div 
        @click="activeTab = 'mirrors'"
        :class="{ 'active': activeTab === 'mirrors' }" 
        class="sidebar-item"
      >镜像源设置</div>
      <div 
        @click="activeTab = 'backup'"
        :class="{ 'active': activeTab === 'backup' }" 
        class="sidebar-item"
      >备份与恢复</div>
      <div class="mt-auto pt-4 border-t border-slate-800 text-sm text-slate-500 text-center">
        v1.0.0-beta
      </div>
    </div>

    <!-- Main Content -->
    <div class="flex-1 flex flex-col overflow-hidden p-8">
      <!-- 1. 环境管理 (Dashboard) -->
      <div v-if="activeTab === 'dashboard'" class="flex-1 flex flex-col overflow-hidden">
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

        <!-- Docker Error Alert -->
        <div v-if="dockerError" class="mb-8 p-6 bg-rose-500/10 border border-rose-500/20 rounded-2xl flex items-center gap-4 text-rose-400">
          <div class="p-3 bg-rose-500/20 rounded-full text-rose-500">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
          </div>
          <div class="flex-1">
            <h3 class="font-bold text-lg mb-1 text-rose-500">Docker 环境异常</h3>
            <p class="text-sm opacity-90">{{ dockerError }}</p>
          </div>
          <button 
            @click="refreshContainers"
            class="px-4 py-2 bg-rose-500 text-white rounded-lg hover:bg-rose-600 transition font-bold text-sm"
          >
            重试
          </button>
        </div>

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
      </div>

      <!-- 2. 虚拟主机 (Vhosts) - Placeholder -->
      <div v-if="activeTab === 'vhosts'" class="flex-1 flex flex-col">
        <h1 class="text-3xl font-bold mb-8">虚拟主机管理</h1>
        <div class="bg-slate-900 border border-slate-800 rounded-2xl p-12 text-center">
          <div class="text-slate-500 mb-4 text-lg">此功能正在快马加鞭开发中...</div>
          <p class="text-slate-600 max-w-md mx-auto">
            后续将支持一键添加 Nginx 站点配置，自动绑定本地域名和项目目录。
          </p>
        </div>
      </div>

      <!-- 3. 软件管理 (Software) - Placeholder -->
      <div v-if="activeTab === 'software'" class="flex-1 flex flex-col">
        <h1 class="text-3xl font-bold mb-8">软件管理 (Docker)</h1>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
          <div class="bg-slate-900 border border-slate-800 rounded-2xl p-6">
            <h3 class="text-xl font-bold mb-4">PHP 版本</h3>
            <div class="space-y-3">
              <div class="flex justify-between items-center p-3 bg-slate-800/50 rounded-lg border border-slate-700/50">
                <span>PHP 8.2</span>
                <button class="px-4 py-1.5 bg-blue-600 text-white rounded-md text-sm">安装</button>
              </div>
              <div class="flex justify-between items-center p-3 bg-slate-800/50 rounded-lg border border-slate-700/50 opacity-50">
                <span>PHP 7.4</span>
                <span class="text-xs italic">待支持</span>
              </div>
            </div>
          </div>
          <div class="bg-slate-900 border border-slate-800 rounded-2xl p-6">
            <h3 class="text-xl font-bold mb-4">其他服务</h3>
            <div class="space-y-3">
              <div class="flex justify-between items-center p-3 bg-slate-800/50 rounded-lg border border-slate-700/50">
                <span>MySQL 5.7</span>
                <button class="px-4 py-1.5 bg-blue-600 text-white rounded-md text-sm">安装</button>
              </div>
              <div class="flex justify-between items-center p-3 bg-slate-800/50 rounded-lg border border-slate-700/50">
                <span>Redis 6.2</span>
                <button class="px-4 py-1.5 bg-blue-600 text-white rounded-md text-sm">安装</button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 4. 镜像源设置 (Mirrors) -->
      <div v-if="activeTab === 'mirrors'" class="flex-1 flex flex-col">
        <h1 class="text-3xl font-bold mb-8">镜像源设置</h1>
        <div class="bg-slate-900 border border-slate-800 rounded-2xl p-8 max-w-2xl">
          <div class="mb-8">
            <label class="block text-sm font-medium text-slate-400 mb-2 uppercase tracking-wider">Docker 镜像加速源</label>
            <div class="flex gap-3">
              <input 
                v-model="dockerMirrorUrl"
                type="text" 
                class="flex-1 bg-slate-800 border border-slate-700 rounded-lg px-4 py-2 focus:ring-2 focus:ring-blue-500 outline-none"
                placeholder="https://..."
              />
              <button 
                @click="updateMirror"
                class="bg-blue-600 hover:bg-blue-700 px-6 py-2 rounded-lg font-medium transition"
              >
                应用
              </button>
            </div>
            <div class="mt-3 flex gap-2">
              <button @click="dockerMirrorUrl = 'https://registry.docker-cn.com'" class="text-xs px-2 py-1 bg-slate-800 rounded hover:bg-slate-700 text-slate-400">官方中国</button>
              <button @click="dockerMirrorUrl = 'https://mirrors.ustc.edu.cn'" class="text-xs px-2 py-1 bg-slate-800 rounded hover:bg-slate-700 text-slate-400">中科大</button>
              <button @click="dockerMirrorUrl = 'https://mirror.ccs.tencentyun.com'" class="text-xs px-2 py-1 bg-slate-800 rounded hover:bg-slate-700 text-slate-400">腾讯云</button>
            </div>
          </div>

          <div>
            <label class="block text-sm font-medium text-slate-400 mb-2 uppercase tracking-wider">PHP 扩展与 Composer 源</label>
            <div class="p-4 bg-slate-800/30 border border-slate-700 rounded-xl space-y-4">
              <div class="flex items-center justify-between">
                <div>
                  <div class="font-medium">Composer 全局源</div>
                  <div class="text-xs text-slate-500 mt-1">设置后将加速 PHP 依赖安装</div>
                </div>
                <select class="bg-slate-800 border border-slate-700 rounded-md px-3 py-1 text-sm outline-none">
                  <option>阿里云 (aliyun)</option>
                  <option>华为云 (huawei)</option>
                  <option>官方 (packagist.org)</option>
                </select>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 5. 备份与恢复 (Backup) - Placeholder -->
      <div v-if="activeTab === 'backup'" class="flex-1 flex flex-col">
        <h1 class="text-3xl font-bold mb-8">备份与恢复</h1>
        <div class="bg-slate-900 border border-slate-800 rounded-2xl p-8">
          <div class="flex items-start gap-6 mb-8">
            <div class="flex-1 p-6 bg-slate-800/30 border border-slate-700 rounded-2xl">
              <h3 class="font-bold text-lg mb-4 flex items-center gap-2">
                <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                一键导出环境
              </h3>
              <div class="space-y-3 mb-6">
                <label class="flex items-center gap-2 text-sm text-slate-400">
                  <input type="checkbox" checked disabled /> PHP-Stack 核心配置
                </label>
                <label class="flex items-center gap-2 text-sm text-slate-400">
                  <input type="checkbox" /> MySQL 数据库 (SQL)
                </label>
                <label class="flex items-center gap-2 text-sm text-slate-400">
                  <input type="checkbox" /> 项目敏感文件 (.env)
                </label>
              </div>
              <button class="w-full py-3 bg-blue-600 hover:bg-blue-700 rounded-xl font-bold transition">
                创建备份包 (.zip)
              </button>
            </div>

            <div class="flex-1 p-6 bg-slate-800/30 border border-slate-700 rounded-2xl">
              <h3 class="font-bold text-lg mb-4 flex items-center gap-2 text-emerald-400">
                <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
                快速恢复环境
              </h3>
              <div class="border-2 border-dashed border-slate-700 rounded-xl p-8 text-center mb-6">
                <p class="text-slate-500 text-sm">将 .zip 备份包拖拽至此处</p>
              </div>
              <button class="w-full py-3 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-xl font-bold transition">
                选择文件
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- Log Panel (Global) -->
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
@reference "tailwindcss";

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
