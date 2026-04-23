<script setup lang="ts">
import { ref, onMounted, nextTick, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import EnvConfigPage from './components/EnvConfigPage.vue';
import SettingsPage from './components/SettingsPage.vue';
import MigrationPage from './components/MigrationPage.vue';
import Toast from './components/Toast.vue';
import ConfirmDialog from './components/ConfirmDialog.vue';
import WorkspaceInitDialog from './components/WorkspaceInitDialog.vue';
import { getLogs, addLog } from './composables/useToast';
import { showConfirm } from './composables/useConfirmDialog';

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
const starting = ref(false); // 启动环境时的加载状态
const logs = getLogs(); // 使用 composable 中的全局日志
const dockerError = ref<string | null>(null);
const activeTab = ref('dashboard');
const showLogs = ref(false); // 控制日志面板显示隐藏（默认隐藏）
const sidebarCollapsed = ref(true); // 控制侧边栏展开/收缩（默认收缩）
const showStartConfirm = ref(false); // 控制启动确认弹窗
const logPanelRef = ref<HTMLElement | null>(null); // 日志面板引用

// 判断容器是否运行中（兼容多种格式）
const isRunning = (state: string): boolean => {
  // 后端返回的格式："Some(RUNNING)" 或 "Some(Exceeded)" 等
  const normalized = state.toLowerCase();
  return normalized.includes('running');
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

const refreshContainers = async (silent = false) => {
  if (!silent) {
    loading.value = true;
    addLog('正在刷新容器状态...');
  }
  if (!(await checkDocker())) {
    containers.value = [];
    if (!silent) {
      loading.value = false;
      addLog('Docker 不可用，已清空容器列表');
    }
    return;
  }
  try {
    const result = await invoke('list_containers') as Container[];
    // 只有当内容真正改变时才更新，减少 DOM 抖动
    if (JSON.stringify(containers.value) !== JSON.stringify(result)) {
      containers.value = result;
      if (!silent) addLog(`容器列表已更新 (${result.length} 个容器)`);
    } else if (!silent) {
      addLog('容器状态未变化');
    }
  } catch (e) {
    if (!silent) addLog(`刷新失败: ${e}`);
  } finally {
    if (!silent) {
      loading.value = false;
      addLog('刷新完成');
    }
  }
};

const startService = async (name: String) => {
  try {
    addLog(`正在启动服务: ${name}...`);
    await invoke('start_container', { name });
    addLog(`服务 ${name} 已启动`);
    await refreshContainers(true); // 使用静默刷新
  } catch (e) {
    addLog(`启动失败: ${e}`);
  }
};

const stopService = async (name: String) => {
  try {
    addLog(`正在停止服务: ${name}...`);
    await invoke('stop_container', { name });
    addLog(`服务 ${name} 已停止`);
    await refreshContainers(true); // 使用静默刷新
  } catch (e) {
    addLog(`停止失败: ${e}`);
  }
};

const openServiceConfig = async (name: String) => {
  try {
    addLog(`正在打开服务配置: ${name}...`);
    // 从容器名称提取服务目录名称
    const containerName = String(name);
    let serviceName = '';
    
    if (containerName.startsWith('ps-php')) {
      // PHP 容器：ps-php56 -> php56, ps-php85 -> php85
      serviceName = containerName.replace('ps-', '');
    } else if (containerName.startsWith('ps-mysql')) {
      // MySQL 容器：ps-mysql57 -> mysql57, ps-mysql84 -> mysql84
      serviceName = containerName.replace('ps-', '');
    } else if (containerName.startsWith('ps-redis')) {
      // Redis 容器：ps-redis62 -> redis62, ps-redis72 -> redis72
      serviceName = containerName.replace('ps-', '');
    } else if (containerName.startsWith('ps-nginx')) {
      // Nginx 容器：ps-nginx127 -> nginx127
      serviceName = containerName.replace('ps-', '');
    } else {
      // 其他情况，尝试去掉 ps- 前缀
      serviceName = containerName.replace(/^ps-/, '');
    }
    
    await invoke('open_service_config', { serviceName });
    addLog(`已打开服务配置目录: ${serviceName}`);
  } catch (e) {
    addLog(`打开配置失败: ${e}`);
  }
};

const handleStartEnvironment = () => {
  showStartConfirm.value = true;
};

const confirmStart = async () => {
  showStartConfirm.value = false;
  
  // 自动打开日志面板
  showLogs.value = true;
  
  starting.value = true;
  addLog('🚀 开始一键启动环境...');
  
  try {
    await invoke('start_environment');
    addLog('✅ 环境启动成功！');
    await refreshContainers();
  } catch (e: any) {
    const errorMsg = String(e);
    
    // 检查是否是端口冲突错误
    if (errorMsg.startsWith('PORT_CONFLICT:')) {
      const conflictDetails = errorMsg.substring('PORT_CONFLICT:'.length);
      const formattedConflicts = conflictDetails.replace(/; /g, '\n• ');
      
      // 显示自定义确认对话框
      const result = await showConfirm({
        title: '⚠️ 检测到端口冲突',
        message: `发现以下端口冲突：\n\n• ${formattedConflicts}\n\n是否继续启动？（可能会导致容器启动失败）`,
        confirmText: '忽略并继续',
        cancelText: '取消启动',
        type: 'warning'
      });
      
      if (result) {
        // 用户选择继续
        addLog('⚠️ 用户选择忽略冲突，继续启动...');
        try {
          await invoke('start_environment');
          addLog('✅ 环境启动成功！');
          await refreshContainers();
        } catch (err) {
          addLog(`❌ 启动失败: ${err}`);
        }
      } else {
        // 用户取消
        addLog('❌ 用户取消启动，请先解决端口冲突');
      }
    } else {
      addLog(`❌ 启动失败: ${e}`);
    }
  } finally {
    starting.value = false;
  }
};

const goToMirrorSettings = () => {
  showStartConfirm.value = false;
  activeTab.value = 'mirrors-unified';
};

onMounted(() => {
  refreshContainers();
  // 每 5 秒自动静默刷新一次
  setInterval(() => refreshContainers(true), 5000);
  
  // 监听后端发送的日志事件
  listen('env-log', (event) => {
    const msg = event.payload as string;
    addLog(msg);
  });
});

// 监听日志变化，自动滚动到底部
watch(logs, async () => {
  await nextTick();
  if (logPanelRef.value) {
    logPanelRef.value.scrollTop = logPanelRef.value.scrollHeight;
  }
}, { deep: true });
</script>

<template>
  <div class="flex h-screen w-screen overflow-hidden bg-slate-950 text-slate-200">
    <!-- Sidebar -->
    <div 
      class="bg-slate-900 flex flex-col border-r border-slate-800 overflow-y-auto transition-all duration-300 ease-in-out"
      :class="sidebarCollapsed ? 'w-20 p-3' : 'w-52 p-4'"
    >
      <!-- Logo -->
      <div class="mb-6 flex items-center gap-2" :class="sidebarCollapsed ? 'justify-center' : ''">
        <span class="bg-blue-500 text-white p-1 rounded font-bold">PS</span>
        <span v-if="!sidebarCollapsed" class="text-2xl font-bold text-blue-400">PHP-Stack</span>
      </div>
      
      <!-- Menu Items -->
      <div class="flex flex-col gap-2">
        <div 
          @click="activeTab = 'dashboard'"
          :class="{ 'active': activeTab === 'dashboard' }" 
          class="sidebar-item"
          :title="sidebarCollapsed ? '环境管理' : ''"
        >
          <span class="text-lg">🏠</span>
          <span v-if="!sidebarCollapsed" class="ml-2">环境管理</span>
        </div>
        <div 
          @click="activeTab = 'env-config'"
          :class="{ 'active': activeTab === 'env-config' }" 
          class="sidebar-item"
          :title="sidebarCollapsed ? '环境配置' : ''"
        >
          <span class="text-lg">🍳</span>
          <span v-if="!sidebarCollapsed" class="ml-2">环境配置</span>
        </div>
        <div 
          @click="activeTab = 'mirrors-unified'"
          :class="{ 'active': activeTab === 'mirrors-unified' }" 
          class="sidebar-item"
          :title="sidebarCollapsed ? '设置项' : ''"
        >
          <span class="text-lg">⚙️</span>
          <span v-if="!sidebarCollapsed" class="ml-2">设置项</span>
        </div>
        <div 
          @click="activeTab = 'migration'"
          :class="{ 'active': activeTab === 'migration' }" 
          class="sidebar-item"
          :title="sidebarCollapsed ? '环境迁移' : ''"
        >
          <span class="text-lg">📦</span>
          <span v-if="!sidebarCollapsed" class="ml-2">环境迁移</span>
        </div>
      </div>
      
      <!-- Version & Toggle Button -->
      <div class="mt-auto pt-4 border-t border-slate-800">
        <div v-if="!sidebarCollapsed" class="text-sm text-slate-500 text-center mb-3">
          v1.0.0-beta
        </div>
        
        <!-- Toggle Button -->
        <button 
          @click="sidebarCollapsed = !sidebarCollapsed"
          class="w-full py-2 px-3 bg-slate-800 hover:bg-slate-700 rounded-lg transition-colors flex items-center justify-center text-slate-400 hover:text-slate-200"
          :title="sidebarCollapsed ? '展开侧边栏' : '收缩侧边栏'"
        >
          <svg 
            xmlns="http://www.w3.org/2000/svg" 
            class="w-5 h-5 transition-transform duration-300"
            :class="sidebarCollapsed ? 'rotate-180' : ''"
            fill="none" 
            viewBox="0 0 24 24" 
            stroke="currentColor" 
            stroke-width="2"
          >
            <path stroke-linecap="round" stroke-linejoin="round" d="M11 19l-7-7 7-7m8 14l-7-7 7-7" />
          </svg>
        </button>
      </div>
    </div>

    <!-- Main Content -->
    <div class="flex-1 flex flex-col overflow-hidden p-4 md:p-6">
      <!-- 1. 环境管理 (Dashboard) -->
      <div v-if="activeTab === 'dashboard'" class="flex-1 flex flex-col overflow-hidden">
        <header class="flex justify-between items-center mb-8">
          <h1 class="text-3xl font-bold">运行状态</h1>
          <div class="flex gap-4">
            <button 
              @click="() => refreshContainers()" 
              :disabled="loading"
              class="bg-blue-600 hover:bg-blue-700 disabled:opacity-50 px-4 py-2 rounded-lg font-medium transition"
            >
              {{ loading ? '刷新中...' : '手动刷新' }}
            </button>
            <button 
              @click="handleStartEnvironment"
              :disabled="loading || starting"
              class="bg-emerald-600 hover:bg-emerald-700 disabled:opacity-50 px-4 py-2 rounded-lg font-medium transition flex items-center gap-2"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>
              {{ starting ? '启动中...' : '一键启动' }}
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
            @click="() => refreshContainers()"
            class="px-4 py-2 bg-rose-500 text-white rounded-lg hover:bg-rose-600 transition font-bold text-sm"
          >
            重试
          </button>
        </div>

        <!-- Container Grid -->
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6 overflow-y-auto mb-8 pr-2">
          <div v-for="c in containers" :key="String(c.id)" class="bg-slate-900 border border-slate-800 rounded-xl p-5 hover:border-blue-500/50 transition-colors shadow-lg">
            <div class="flex justify-between items-start mb-4">
              <span class="text-slate-400 text-xs font-mono uppercase tracking-wider">{{ String(c.image).split(':')[0] }}</span>
              <span 
                :class="isRunning(String(c.state)) ? 'text-emerald-400' : 'text-rose-400'"
                class="flex items-center gap-1.5 text-xs font-bold uppercase tracking-tighter"
              >
                <span :class="isRunning(String(c.state)) ? 'bg-emerald-500' : 'bg-rose-500'" class="w-2 h-2 rounded-full animate-pulse"></span>
                {{ isRunning(String(c.state)) ? 'Running' : 'Stopped' }}
              </span>
            </div>
            <div class="text-xl font-bold mb-1 truncate" :title="String(c.name)">{{ String(c.name) }}</div>
            <div class="text-slate-500 text-xs mb-4">
              <span v-if="c.ports.length > 0">Ports: {{ c.ports.join(', ') }}</span>
              <span v-else>No public ports</span>
            </div>
            
            <div class="flex gap-2">
              <button 
                v-if="!isRunning(String(c.state))"
                @click="startService(String(c.name))"
                class="flex-1 py-2 bg-emerald-600/20 hover:bg-emerald-600 text-emerald-400 hover:text-white border border-emerald-600/30 rounded text-sm font-medium transition-all"
              >
                启动
              </button>
              <button 
                v-else
                @click="stopService(String(c.name))"
                class="flex-1 py-2 bg-rose-600/20 hover:bg-rose-600 text-rose-400 hover:text-white border border-rose-600/30 rounded text-sm font-medium transition-all"
              >
                停止
              </button>
              <button 
                @click="openServiceConfig(String(c.name))"
                class="px-3 py-2 bg-slate-800 hover:bg-slate-700 rounded text-sm transition border border-slate-700"
              >
                配置
              </button>
            </div>
          </div>

          <!-- Empty State -->
          <div v-if="containers.length === 0 && !loading" class="col-span-full py-20 text-center bg-slate-900/50 border-2 border-dashed border-slate-800 rounded-2xl">
            <div class="text-slate-500 mb-2">未发现 ps- 前缀的容器</div>
            <div class="text-slate-600 text-sm">请先在“环境配置”中配置 PHP、Nginx 等环境</div>
          </div>
        </div>
      </div>

      <!-- 环境配置 (EnvConfig) -->
      <div v-if="activeTab === 'env-config'" class="flex-1 flex flex-col overflow-hidden">
        <EnvConfigPage @request-switch-tab="(tab) => activeTab = tab" />
      </div>

      <!-- New: 设置项 (SettingsPage) -->
      <div v-if="activeTab === 'mirrors-unified'" class="flex-1 flex flex-col overflow-hidden">
        <SettingsPage />
      </div>

      <!-- New: 环境迁移 (MigrationPage) -->
      <div v-if="activeTab === 'migration'" class="flex-1 flex flex-col overflow-hidden">
        <MigrationPage />
      </div>

      <!-- Log Panel (Global) -->
      <div class="mt-auto border-t border-slate-800 pt-4 bg-slate-950/50 backdrop-blur-md">
        <div class="flex justify-between items-center mb-3">
          <h2 class="text-lg font-bold flex items-center gap-2 text-slate-400">
            <span class="w-2 h-2 bg-blue-500 rounded-full" :class="{ 'animate-pulse': loading }"></span> 
            实时日志
          </h2>
          <button 
            @click="showLogs = !showLogs"
            class="text-xs px-2 py-1 bg-slate-800 hover:bg-slate-700 rounded text-slate-400 transition-colors flex items-center gap-1"
          >
            {{ showLogs ? '隐藏' : '显示' }}
            <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path v-if="showLogs" d="m6 9 6 6 6-6"/><path v-else d="m18 15-6-6-6 6"/>
            </svg>
          </button>
        </div>
        
        <transition name="fade">
          <div 
            v-show="showLogs" 
            ref="logPanelRef"
            class="bg-black/40 p-4 rounded-xl font-mono text-sm text-blue-300/80 border border-slate-800 h-40 overflow-y-auto scrollbar-hide shadow-inner overflow-hidden"
          >
            <div v-for="(log, i) in logs" :key="i" class="mb-1 last:mb-0 animate-in fade-in slide-in-from-left-2 duration-300">
              {{ log }}
            </div>
            <div v-if="logs.length === 0" class="text-slate-600 italic">等待操作中...</div>
          </div>
        </transition>
      </div>
    </div>

    <!-- Global Toast -->
    <Toast />
    
    <!-- Global Confirm Dialog -->
    <ConfirmDialog />
    
    <!-- Workspace Initialization Dialog -->
    <WorkspaceInitDialog />

    <!-- Start Environment Confirmation Dialog -->
    <div v-if="showStartConfirm" class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50">
      <div class="bg-slate-900 border border-slate-700 rounded-xl p-8 max-w-md w-full shadow-2xl">
        <h2 class="text-2xl font-bold text-white mb-4">⚠️ 启动环境提示</h2>
        <p class="text-slate-400 mb-6">
          在启动过程中，Docker 可能需要从网络拉取镜像或下载扩展脚本。
          如果遇到网络连接问题（如 GitHub 访问失败），建议您先配置 <strong>GITHUB_PROXY</strong>。
        </p>

        <div class="space-y-4">
          <div class="flex gap-3">
            <button 
              @click="showStartConfirm = false"
              class="flex-1 px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-lg font-medium transition"
            >
              取消
            </button>
            <button 
              @click="goToMirrorSettings"
              class="flex-1 px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition"
            >
              去配置镜像源
            </button>
          </div>
          <button 
            @click="confirmStart"
            class="w-full px-6 py-2 bg-emerald-600 hover:bg-emerald-700 text-white rounded-lg font-bold transition shadow-lg shadow-emerald-600/20"
          >
            直接启动
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
@reference "tailwindcss";

.sidebar-item {
  @apply px-4 py-3 rounded-lg transition-all cursor-pointer text-slate-400 hover:bg-slate-800 hover:text-slate-100 border border-transparent flex items-center;
}
.sidebar-item.active {
  @apply bg-blue-600/10 text-blue-400 border-blue-600/20;
}
.scrollbar-hide::-webkit-scrollbar {
  display: none;
}

/* 日志面板切换动画 */
.fade-enter-active,
.fade-leave-active {
  transition: all 0.3s ease;
  max-height: 160px;
  opacity: 1;
}

.fade-enter-from,
.fade-leave-to {
  max-height: 0;
  opacity: 0;
  padding-top: 0;
  padding-bottom: 0;
  margin-top: 0;
  overflow: hidden;
}
</style>

<!-- 全局滚动条样式 -->
<style>
/* Webkit 浏览器滚动条 (Chrome, Safari, Edge) */
::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

::-webkit-scrollbar-track {
  background: rgba(30, 41, 59, 0.3); /* slate-900 with opacity */
  border-radius: 4px;
}

::-webkit-scrollbar-thumb {
  background: rgba(71, 85, 105, 0.6); /* slate-600 with opacity */
  border-radius: 4px;
  transition: background 0.2s ease;
}

::-webkit-scrollbar-thumb:hover {
  background: rgba(100, 116, 139, 0.8); /* slate-500 with opacity */
}

::-webkit-scrollbar-thumb:active {
  background: rgba(148, 163, 184, 0.9); /* slate-400 with opacity */
}

/* Firefox 滚动条 */
* {
  scrollbar-width: thin;
  scrollbar-color: rgba(71, 85, 105, 0.6) rgba(30, 41, 59, 0.3);
}
</style>
