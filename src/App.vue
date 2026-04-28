<script setup lang="ts">
import { ref, onMounted, nextTick, watch, computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import EnvConfigPage from './components/EnvConfigPage.vue';
import SettingsPage from './components/SettingsPage.vue';
import MigrationPage from './components/MigrationPage.vue';
import Toast from './components/Toast.vue';
import ConfirmDialog from './components/ConfirmDialog.vue';
import WorkspaceInitDialog from './components/WorkspaceInitDialog.vue';
import { getLogs, addLog, showToast } from './composables/useToast';
import { showConfirm } from './composables/useConfirmDialog';

const { t } = useI18n();

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
const operationType = ref<'start' | 'restart' | 'stop' | null>(null); // 当前操作类型
const logs = getLogs(); // 使用 composable 中的全局日志
const dockerError = ref<string | null>(null);
const activeTab = ref('dashboard');
const showLogs = ref(false); // 控制日志面板显示隐藏（默认隐藏）
const sidebarCollapsed = ref(window.innerWidth < 768); // 控制侧边栏展开/收缩（小屏幕默认收缩）
const showStartConfirm = ref(false); // 控制启动确认弹窗
const showRestartConfirm = ref(false); // 控制重启确认弹窗
const logPanelRef = ref<HTMLElement | null>(null); // 日志面板引用
const isUserScrolling = ref(false); // 用户是否正在手动滚动
let scrollTimeout: ReturnType<typeof setTimeout> | null = null; // 滚动超时定时器
const hasEnvFile = ref(false); // .env 文件是否存在

// 判断是否有运行中的 ps- 容器
const hasRunningContainers = computed(() => {
  return containers.value.some(c => isRunning(String(c.state)));
});

// 判断是否有任何 ps- 容器（不管状态）
// @ts-ignore - 用于后续功能扩展，暂时未使用
const hasAnyContainers = computed(() => {
  return containers.value.length > 0;
});

// 判断是否有任何停止的 ps- 容器
// @ts-ignore - 用于后续功能扩展，暂时未使用
const hasStoppedContainers = computed(() => {
  return containers.value.some(c => !isRunning(String(c.state)));
});

// 判断是否可以启动（没有任何容器或所有容器都已停止，且存在 .env 文件）
const canStart = computed(() => {
  return !hasRunningContainers.value && hasEnvFile.value;
});

// 判断是否可以重启（有运行中的容器）
const canRestart = computed(() => {
  return hasRunningContainers.value;
});

// 判断是否可以停止（有运行中的容器）
const canStop = computed(() => {
  return hasRunningContainers.value;
});

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
    addLog(t('dashboard.toast.dockerCheckFailed', { error: e }));
    return false;
  }
};

const refreshContainers = async (silent = false) => {
  if (!silent) {
    loading.value = true;
    addLog(t('dashboard.toast.refreshing'));
  }
  if (!(await checkDocker())) {
    containers.value = [];
    if (!silent) {
      loading.value = false;
      addLog(t('dashboard.toast.dockerUnavailable'));
    }
    return;
  }
  try {
    const result = await invoke('list_containers') as Container[];
    
    // 只有当内容真正改变时才更新，减少 DOM 抖动
    if (JSON.stringify(containers.value) !== JSON.stringify(result)) {
      containers.value = result;
      if (!silent) addLog(t('dashboard.toast.containerUpdated', { count: result.length }));
    } else if (!silent) {
      addLog(t('dashboard.toast.containerNoChange'));
    }
  } catch (e) {
    if (!silent) addLog(t('dashboard.toast.refreshFailed', { error: e }));
  } finally {
    if (!silent) {
      loading.value = false;
      addLog(t('dashboard.toast.refreshDone'));
    }
  }
};

const startService = async (name: String) => {
  try {
    addLog(t('dashboard.toast.serviceStarting', { name }));
    await invoke('start_container', { name });
    addLog(t('dashboard.toast.serviceStarted', { name }));
    await refreshContainers(true);
  } catch (e) {
    addLog(t('dashboard.toast.serviceStartFailed', { error: e }));
  }
};

const stopService = async (name: String) => {
  try {
    addLog(t('dashboard.toast.serviceStopping', { name }));
    await invoke('stop_container', { name });
    addLog(t('dashboard.toast.serviceStopped', { name }));
    await refreshContainers(true);
  } catch (e) {
    addLog(t('dashboard.toast.serviceStopFailed', { error: e }));
  }
};

const openServiceConfig = async (name: String) => {
  try {
    addLog(t('dashboard.toast.configOpening', { name }));
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
    addLog(t('dashboard.toast.configOpened', { name: serviceName }));
  } catch (e) {
    addLog(t('dashboard.toast.configOpenFailed', { error: e }));
  }
};

const handleStartEnvironment = () => {
  showStartConfirm.value = true;
};

const handleRestartEnvironment = () => {
  showRestartConfirm.value = true;
};

const handleStopEnvironment = async () => {
  // 自动打开日志面板
  showLogs.value = true;
  
  operationType.value = 'stop';
  starting.value = true;
  addLog(t('dashboard.toast.envStopping'));
  
  try {
    await invoke('stop_environment');
    addLog(t('dashboard.toast.envStopped'));
    
    // 等待 1 秒让 Docker API 状态更新
    await new Promise(resolve => setTimeout(resolve, 1000));
    
    await refreshContainers();
  } catch (e: any) {
    addLog(t('dashboard.toast.envStopFailed', { error: e }));
  } finally {
    starting.value = false;
    operationType.value = null;
  }
};

const confirmStart = async () => {
  showStartConfirm.value = false;
  
  // 自动打开日志面板
  showLogs.value = true;
  
  operationType.value = 'start';
  starting.value = true;
  addLog(t('dashboard.toast.envStarting'));
  
  try {
    await invoke('start_environment');
    addLog(t('dashboard.toast.envStarted'));
    
    // 等待 1 秒让 Docker API 状态更新
    await new Promise(resolve => setTimeout(resolve, 1000));
    
    await refreshContainers();
  } catch (e: any) {
    const errorMsg = String(e);
    
    // 检查是否是端口冲突错误
    if (errorMsg.startsWith('PORT_CONFLICT:')) {
      const conflictDetails = errorMsg.substring('PORT_CONFLICT:'.length);
      const formattedConflicts = conflictDetails.replace(/; /g, '\n• ');
      
      // 显示自定义确认对话框
      const result = await showConfirm({
        title: t('dashboard.portConflict.title'),
        message: t('dashboard.portConflict.message', { details: `• ${formattedConflicts}` }),
        confirmText: t('dashboard.portConflict.continue'),
        cancelText: t('dashboard.portConflict.cancel'),
        type: 'warning'
      });
      
      if (result) {
        // 用户选择继续
        addLog(t('dashboard.toast.portConflictIgnore'));
        try {
          await invoke('start_environment');
          addLog(t('dashboard.toast.envStarted'));
          
          // 等待 1 秒让 Docker API 状态更新
          await new Promise(resolve => setTimeout(resolve, 1000));
          
          await refreshContainers();
        } catch (err) {
          addLog(t('dashboard.toast.envStartFailed', { error: err }));
        }
      } else {
        // 用户取消
        addLog(t('dashboard.toast.portConflictCancel'));
      }
    } else {
      addLog(t('dashboard.toast.envStartFailed', { error: e }));
    }
  } finally {
    starting.value = false;
    operationType.value = null;
  }
};

const confirmRestart = async () => {
  showRestartConfirm.value = false;
  
  // 自动打开日志面板
  showLogs.value = true;
  
  operationType.value = 'restart';
  starting.value = true;
  addLog(t('dashboard.toast.envRestarting'));
  
  try {
    await invoke('restart_environment');
    addLog(t('dashboard.toast.envRestarted'));
    
    // 等待 1 秒让 Docker API 状态更新
    await new Promise(resolve => setTimeout(resolve, 1000));
    
    await refreshContainers();
  } catch (e: any) {
    addLog(t('dashboard.toast.envRestartFailed', { error: e }));
  } finally {
    starting.value = false;
    operationType.value = null;
  }
};

const goToMirrorSettings = () => {
  showStartConfirm.value = false;
  activeTab.value = 'mirrors-unified';
};

// 检查 .env 文件是否存在
const checkEnvFileExists = async () => {
  try {
    const existingFiles = await invoke<string[]>('check_config_files_exist');
    hasEnvFile.value = existingFiles.some(f => f.includes('.env'));
    console.log('[App] .env 文件存在:', hasEnvFile.value);
  } catch (e) {
    console.error('[App] 检查配置文件失败:', e);
    hasEnvFile.value = false;
  }
};

// 监听 tab 切换，回到 dashboard 时刷新 .env 检测状态
watch(activeTab, async (newTab) => {
  if (newTab === 'dashboard') {
    await checkEnvFileExists();
  }
});

onMounted(() => {
  refreshContainers();
  checkEnvFileExists(); // 检查 .env 文件是否存在
  // 每 5 秒自动静默刷新一次
  setInterval(() => refreshContainers(true), 5000);
  
  // 监听后端发送的日志事件
  listen('env-log', (event) => {
    const msg = event.payload as string;
    addLog(msg);
  });
});

// 监听日志变化，自动滚动到底部（用户未手动滚动时）
watch(logs, async () => {
  await nextTick();
  if (logPanelRef.value && !isUserScrolling.value) {
    logPanelRef.value.scrollTop = logPanelRef.value.scrollHeight;
  }
}, { deep: true });

// 监听日志面板显示状态，显示时自动滚动到底部
watch(showLogs, async (newValue) => {
  if (newValue) {
    await nextTick();
    if (logPanelRef.value) {
      logPanelRef.value.scrollTop = logPanelRef.value.scrollHeight;
    }
  }
});

// 处理用户手动滚动
const handleLogScroll = () => {
  isUserScrolling.value = true;
  
  // 清除之前的定时器
  if (scrollTimeout) {
    clearTimeout(scrollTimeout);
  }
  
  // 1秒后恢复自动滚动（缩短等待时间）
  scrollTimeout = setTimeout(() => {
    isUserScrolling.value = false;
  }, 1000);
};

// 手动滚动到底部
const scrollToBottom = async () => {
  await nextTick();
  if (logPanelRef.value) {
    logPanelRef.value.scrollTop = logPanelRef.value.scrollHeight;
    isUserScrolling.value = false; // 重置手动滚动状态
  }
};

// 复制日志到剪贴板
async function copyLogs() {
  try {
    const logs = await invoke('export_logs');
    await writeText(logs as string);
    showToast(t('dashboard.log.copied'), 'success');
  } catch (e) {
    showToast(t('dashboard.log.copyFailed', { error: e }), 'error');
  }
}
</script>

<template>
  <div class="flex h-screen w-screen overflow-hidden bg-slate-950 text-slate-200">
    <!-- Sidebar -->
    <div 
      class="bg-slate-900 flex flex-col border-r border-slate-800 overflow-y-auto transition-all duration-300 ease-in-out"
      :class="sidebarCollapsed ? 'w-16 sm:w-20 p-2 sm:p-3' : 'w-48 sm:w-52 p-3 sm:p-4'"
    >
      <!-- Logo -->
      <div class="mb-4 sm:mb-6 flex items-center gap-2" :class="sidebarCollapsed ? 'justify-center' : ''">
        <span class="bg-blue-500 text-white p-1 rounded font-bold text-sm sm:text-base">PS</span>
        <span v-if="!sidebarCollapsed" class="text-xl sm:text-2xl font-bold text-blue-400 hidden sm:inline">PHP-Stack</span>
      </div>
      
      <!-- Menu Items -->
      <div class="flex flex-col gap-2">
        <div 
          @click="activeTab = 'dashboard'"
          :class="{ 'active': activeTab === 'dashboard' }" 
          class="sidebar-item text-sm sm:text-base"
          :title="sidebarCollapsed ? $t('sidebar.dashboard') : ''"
        >
          <span class="text-base sm:text-lg">🏠</span>
          <span v-if="!sidebarCollapsed" class="ml-2 hidden sm:inline">{{ $t('sidebar.dashboard') }}</span>
        </div>
        <div 
          @click="activeTab = 'env-config'"
          :class="{ 'active': activeTab === 'env-config' }" 
          class="sidebar-item text-sm sm:text-base"
          :title="sidebarCollapsed ? $t('sidebar.envConfig') : ''"
        >
          <span class="text-base sm:text-lg">🛠️</span>
          <span v-if="!sidebarCollapsed" class="ml-2 hidden sm:inline">{{ $t('sidebar.envConfig') }}</span>
        </div>
        <div 
          @click="activeTab = 'mirrors-unified'"
          :class="{ 'active': activeTab === 'mirrors-unified' }" 
          class="sidebar-item text-sm sm:text-base"
          :title="sidebarCollapsed ? $t('sidebar.settings') : ''"
        >
          <span class="text-base sm:text-lg">⚙️</span>
          <span v-if="!sidebarCollapsed" class="ml-2 hidden sm:inline">{{ $t('sidebar.settings') }}</span>
        </div>
        <div 
          @click="activeTab = 'migration'"
          :class="{ 'active': activeTab === 'migration' }" 
          class="sidebar-item text-sm sm:text-base"
          :title="sidebarCollapsed ? $t('sidebar.migration') : ''"
        >
          <span class="text-base sm:text-lg">📦</span>
          <span v-if="!sidebarCollapsed" class="ml-2 hidden sm:inline">{{ $t('sidebar.migration') }}</span>
        </div>
      </div>
      
      <!-- Version & Toggle Button -->
      <div class="mt-auto pt-3 sm:pt-4 border-t border-slate-800">
        <div v-if="!sidebarCollapsed" class="text-xs sm:text-sm text-slate-500 text-center mb-2 sm:mb-3 hidden sm:block">
          {{ $t('common.version') }}
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
    <div class="flex-1 flex flex-col overflow-hidden p-3 sm:p-4 md:p-5 lg:p-6">
      <!-- 1. 环境管理 (Dashboard) -->
      <div v-if="activeTab === 'dashboard'" class="flex-1 flex flex-col overflow-hidden">
        <header class="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-4 mb-6 sm:mb-8">
          <h1 class="text-3xl font-bold">{{ $t('dashboard.title') }}</h1>
          <div class="flex flex-col sm:flex-row gap-3 w-full sm:w-auto">
            <button 
              @click="() => refreshContainers()" 
              :disabled="loading"
              class="w-full sm:w-auto bg-blue-600 hover:bg-blue-700 disabled:opacity-50 px-4 py-2 rounded-lg font-medium transition"
            >
              {{ loading ? $t('dashboard.refreshing') : $t('dashboard.refresh') }}
            </button>
            <div class="flex flex-col sm:flex-row gap-3 w-full sm:w-auto">
              <button 
                @click="handleStartEnvironment"
                :disabled="!canStart || starting"
                class="w-full sm:w-auto bg-emerald-600 hover:bg-emerald-700 disabled:opacity-50 disabled:cursor-not-allowed px-4 py-2 rounded-lg font-medium transition flex items-center justify-center gap-2"
                :title="!hasEnvFile ? $t('dashboard.startTooltip.noEnv') : (!canStart ? $t('dashboard.startTooltip.hasRunning') : '')"
              >
                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>
                {{ operationType === 'start' ? $t('dashboard.startingEnv') : $t('dashboard.startEnv') }}
              </button>
              <button 
                @click="handleRestartEnvironment"
                :disabled="!canRestart || starting"
                class="w-full sm:w-auto bg-amber-600 hover:bg-amber-700 disabled:opacity-50 disabled:cursor-not-allowed px-4 py-2 rounded-lg font-medium transition flex items-center justify-center gap-2"
                :title="!canRestart ? $t('dashboard.restartTooltip') : ''"
              >
                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"></polyline><polyline points="1 20 1 14 7 14"></polyline><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path></svg>
                {{ operationType === 'restart' ? $t('dashboard.restartingEnv') : $t('dashboard.restartEnv') }}
              </button>
              <button 
                @click="handleStopEnvironment"
                :disabled="!canStop || starting"
                class="w-full sm:w-auto bg-rose-600 hover:bg-rose-700 disabled:opacity-50 disabled:cursor-not-allowed px-4 py-2 rounded-lg font-medium transition flex items-center justify-center gap-2"
                :title="!canStop ? $t('dashboard.stopTooltip') : ''"
              >
                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="6" y="6" width="12" height="12"></rect></svg>
                {{ operationType === 'stop' ? $t('dashboard.stoppingEnv') : $t('dashboard.stopEnv') }}
              </button>
            </div>
          </div>
        </header>

        <!-- Docker Error Alert -->
        <div v-if="dockerError" class="mb-8 p-6 bg-rose-500/10 border border-rose-500/20 rounded-2xl flex items-center gap-4 text-rose-400">
          <div class="p-3 bg-rose-500/20 rounded-full text-rose-500">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
          </div>
          <div class="flex-1">
            <h3 class="font-bold text-lg mb-1 text-rose-500">{{ $t('dashboard.dockerError.title') }}</h3>
            <p class="text-sm opacity-90">{{ dockerError }}</p>
          </div>
          <button 
            @click="() => refreshContainers()"
            class="px-4 py-2 bg-rose-500 text-white rounded-lg hover:bg-rose-600 transition font-bold text-sm"
          >
            {{ $t('dashboard.dockerError.retry') }}
          </button>
        </div>

        <!-- No Env File Alert -->
        <div v-if="!hasEnvFile" class="mb-8 p-6 bg-amber-500/10 border border-amber-500/20 rounded-2xl flex items-center gap-4 text-amber-400">
          <div class="p-3 bg-amber-500/20 rounded-full text-amber-500">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
          </div>
          <div class="flex-1">
            <h3 class="font-bold text-lg mb-1 text-amber-500">{{ $t('dashboard.noEnvFile.title') }}</h3>
            <p class="text-sm opacity-90">{{ $t('dashboard.noEnvFile.description') }}</p>
          </div>
          <button 
            @click="activeTab = 'env-config'"
            class="px-4 py-2 bg-amber-500 text-white rounded-lg hover:bg-amber-600 transition font-bold text-sm whitespace-nowrap"
          >
            {{ $t('dashboard.noEnvFile.action') }}
          </button>
        </div>

        <!-- Container Grid -->
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4 sm:gap-6 overflow-y-auto mb-8 pr-2">
          <div v-for="c in containers" :key="String(c.id)" class="bg-slate-900 border border-slate-800 rounded-xl p-5 hover:border-blue-500/50 transition-colors shadow-lg">
            <div class="flex justify-between items-start mb-4">
              <span class="text-slate-400 text-xs font-mono uppercase tracking-wider">{{ String(c.image).split(':')[0] }}</span>
              <span 
                :class="isRunning(String(c.state)) ? 'text-emerald-400' : 'text-rose-400'"
                class="flex items-center gap-1.5 text-xs font-bold uppercase tracking-tighter"
              >
                <span :class="isRunning(String(c.state)) ? 'bg-emerald-500' : 'bg-rose-500'" class="w-2 h-2 rounded-full animate-pulse"></span>
                {{ isRunning(String(c.state)) ? $t('dashboard.container.running') : $t('dashboard.container.stopped') }}
              </span>
            </div>
            <div class="text-xl font-bold mb-1 truncate" :title="String(c.name)">{{ String(c.name) }}</div>
            <div class="text-slate-500 text-xs mb-4">
              <span v-if="c.ports.length > 0">Ports: {{ c.ports.join(', ') }}</span>
              <span v-else>{{ $t('dashboard.container.noPorts') }}</span>
            </div>
            
            <div class="flex gap-2">
              <button 
                v-if="!isRunning(String(c.state))"
                @click="startService(String(c.name))"
                class="flex-1 py-2 bg-emerald-600/20 hover:bg-emerald-600 text-emerald-400 hover:text-white border border-emerald-600/30 rounded text-sm font-medium transition-all"
              >
                {{ $t('dashboard.container.start') }}
              </button>
              <button 
                v-else
                @click="stopService(String(c.name))"
                class="flex-1 py-2 bg-rose-600/20 hover:bg-rose-600 text-rose-400 hover:text-white border border-rose-600/30 rounded text-sm font-medium transition-all"
              >
                {{ $t('dashboard.container.stop') }}
              </button>
              <button 
                @click="openServiceConfig(String(c.name))"
                class="px-3 py-2 bg-slate-800 hover:bg-slate-700 rounded text-sm transition border border-slate-700"
              >
                {{ $t('dashboard.container.config') }}
              </button>
            </div>
          </div>

          <!-- Empty State -->
          <div v-if="containers.length === 0 && !loading" class="col-span-full py-20 text-center bg-slate-900/50 border-2 border-dashed border-slate-800 rounded-2xl">
            <div class="text-slate-500 mb-2">{{ t('dashboard.empty.title') }}</div>
            <div class="text-slate-600 text-sm">{{ t('dashboard.empty.description') }}</div>
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
      <div class="mt-auto border-t border-slate-800 pt-3 sm:pt-4 bg-slate-950/50 backdrop-blur-md">
        <div class="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-2 sm:gap-3 mb-2 sm:mb-3">
          <h2 class="text-lg font-bold flex items-center gap-2 text-slate-400">
            <span class="w-2 h-2 bg-blue-500 rounded-full" :class="{ 'animate-pulse': loading }"></span> 
            {{ $t('dashboard.log.title') }}
          </h2>
          <div class="flex flex-wrap gap-2">
            <button 
              @click="copyLogs"
              class="text-xs px-2 py-1 bg-slate-800 hover:bg-slate-700 rounded text-slate-400 transition-colors flex items-center gap-1"
              title="复制日志到剪贴板"
            >
              📋 {{ $t('common.copy') }}
            </button>
            <button 
              @click="scrollToBottom"
              class="text-xs px-2 py-1 bg-slate-800 hover:bg-slate-700 rounded text-slate-400 transition-colors flex items-center gap-1"
              title=""
            >
              {{ $t('dashboard.log.bottom') }}
            </button>
            <button 
              @click="showLogs = !showLogs"
              class="text-xs px-2 py-1 bg-slate-800 hover:bg-slate-700 rounded text-slate-400 transition-colors flex items-center gap-1"
            >
              {{ showLogs ? $t('dashboard.log.hide') : $t('dashboard.log.show') }}
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path v-if="showLogs" d="m6 9 6 6 6-6"/><path v-else d="m18 15-6-6-6 6"/>
              </svg>
            </button>
          </div>
        </div>
        
        <transition name="fade">
          <div 
            v-show="showLogs" 
            ref="logPanelRef"
            @scroll="handleLogScroll"
            class="bg-black/40 p-3 sm:p-4 rounded-xl font-mono text-xs sm:text-sm text-blue-300/80 border border-slate-800 h-32 sm:h-40 overflow-y-auto scrollbar-hide shadow-inner overflow-hidden"
          >
            <div v-for="(log, i) in logs" :key="i" class="mb-1 last:mb-0 animate-in fade-in slide-in-from-left-2 duration-300">
              {{ log }}
            </div>
            <div v-if="logs.length === 0" class="text-slate-600 italic">{{ $t('dashboard.log.empty') }}</div>
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
        <h2 class="text-2xl font-bold text-white mb-4">{{ $t('dashboard.startConfirm.title') }}</h2>
        <p class="text-slate-400 mb-6">
          {{ $t('dashboard.startConfirm.message', { proxy: '' }) }}
          <strong>{{ $t('dashboard.startConfirm.proxy') }}</strong>
        </p>

        <div class="space-y-4">
          <div class="flex gap-3">
            <button 
              @click="showStartConfirm = false"
              class="flex-1 px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-lg font-medium transition"
            >
              {{ $t('common.cancel') }}
            </button>
            <button 
              @click="goToMirrorSettings"
              class="flex-1 px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition"
            >
              {{ $t('dashboard.startConfirm.goMirror') }}
            </button>
          </div>
          <button 
            @click="confirmStart"
            class="w-full px-6 py-2 bg-emerald-600 hover:bg-emerald-700 text-white rounded-lg font-bold transition shadow-lg shadow-emerald-600/20"
          >
            {{ $t('dashboard.startConfirm.directStart') }}
          </button>
        </div>
      </div>
    </div>

    <!-- Restart Environment Confirmation Dialog -->
    <div v-if="showRestartConfirm" class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50">
      <div class="bg-slate-900 border border-slate-700 rounded-xl p-8 max-w-md w-full shadow-2xl">
        <h2 class="text-2xl font-bold text-white mb-4">{{ $t('dashboard.restartConfirm.title') }}</h2>
        <p class="text-slate-400 mb-6">
          {{ $t('dashboard.restartConfirm.message', { warning: '' }) }}
          <strong class="text-amber-400">{{ $t('dashboard.restartConfirm.warning') }}</strong>
        </p>
        <div class="space-y-4">
          <button 
            @click="showRestartConfirm = false"
            class="w-full px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-lg font-medium transition"
          >
            {{ $t('common.cancel') }}
          </button>
          <button 
            @click="confirmRestart"
            class="w-full px-6 py-2 bg-amber-600 hover:bg-amber-700 text-white rounded-lg font-bold transition shadow-lg shadow-amber-600/20"
          >
            {{ $t('dashboard.restartConfirm.action') }}
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
