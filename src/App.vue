<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick, watch, computed } from 'vue';
import { useI18n } from 'vue-i18n';
import {
  checkDocker as checkDockerApi,
  listContainers,
  startContainer,
  stopContainer,
  openServiceConfig as openServiceConfigApi,
  startEnvironment,
  stopEnvironment,
  restartEnvironment,
  getWorkspaceInfo,
  checkConfigFilesExist,
  exportLogsTo,
  normalizeError,
  isPortConflictError,
  stripProtocolPrefix,
  PORT_CONFLICT_PREFIX,
  type WorkspaceInfo,
} from './api';
import { listen } from '@tauri-apps/api/event';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { save } from '@tauri-apps/plugin-dialog';
import { getVersion } from '@tauri-apps/api/app';
import EnvConfigPage from './components/EnvConfigPage.vue';
import SettingsPage from './components/SettingsPage.vue';
import MigrationPage from './components/MigrationPage.vue';
import AboutPage from './components/AboutPage.vue';
import Toast from './components/Toast.vue';
import ConfirmDialog from './components/ConfirmDialog.vue';
import WorkspaceInitDialog from './components/WorkspaceInitDialog.vue';
import WorkspaceMissingDialog from './components/WorkspaceMissingDialog.vue';
import { addLog, addLogKey, clearLogs, showToast, UI_LOG_LIMIT, visibleLogs, formatLogLine } from './composables/useToast';
import { showConfirm } from './composables/useConfirmDialog';
import { pendingUpdateVersion, setPendingUpdateVersion } from './composables/useUpdater';
import type { Container } from './types/docker';
import { isContainerRunning } from './types/docker';
import { nextPollDelay, POLL_INTERVAL_MS } from './utils/pollBackoff';
import { WORKSPACE_CHANGED_EVENT } from './utils/workspaceEvents';

const { t } = useI18n();

const appVersion = ref('v0.0.0'); // 应用版本号

const containers = ref<Container[]>([]);
const loading = ref(false);
const starting = ref(false); // 启动环境时的加载状态
const operationType = ref<'start' | 'restart' | 'stop' | null>(null); // 当前操作类型
const logs = visibleLogs;
const dockerError = ref<string | null>(null);
const activeTab = ref('dashboard');
const showLogs = ref(false); // 控制日志面板显示隐藏（默认隐藏）
const sidebarCollapsed = ref(window.innerWidth < 768); // 控制侧边栏展开/收缩（小屏幕默认收缩）
const showStartConfirm = ref(false); // 控制启动确认弹窗
const showRestartConfirm = ref(false); // 控制重启确认弹窗
const logPanelRef = ref<HTMLElement | null>(null); // 日志面板引用
const isUserScrolling = ref(false); // 用户是否正在手动滚动
let scrollTimeout: ReturnType<typeof setTimeout> | null = null; // 滚动超时定时器
let consecutiveFailures = 0; // Docker 连续失败次数（用于轮询退避）
let pollTimer: ReturnType<typeof setTimeout> | null = null; // 轮询定时器
const hasEnvFile = ref(false); // .env 文件是否存在
/// 工作区回退告警（配置路径不可用、数据写到别处）。全局横幅展示，不限环境配置页。
const workspaceFallbackMsg = ref('');
/// 配置路径不存在时弹出三选一对话框
const showWorkspaceMissing = ref(false);
const workspaceMissingInfo = ref<Pick<WorkspaceInfo, 'workspace_path' | 'effective_path'> | null>(null);
/// 本会话已选「临时回退」则不再反复弹窗（横幅仍保留）
const workspaceMissingDismissed = ref(false);


// 判断是否有运行中的 ps- 容器
const hasRunningContainers = computed(() => {
  return containers.value.some(c => isContainerRunning(c.state));
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

const checkDocker = async () => {
  try {
    await checkDockerApi();
    // Docker 刚恢复可用时才提示——持续不可用时每次轮询都刷一条毫无意义
    if (dockerError.value !== null) {
      addLogKey('dashboard.toast.dockerRestored');
    }
    dockerError.value = null;
    return true;
  } catch (e) {
    // 只在状态由可用翻转为不可用时记一条，之后静默退避
    const wasAvailable = dockerError.value === null;
    dockerError.value = e as string;
    if (wasAvailable) {
      addLogKey('dashboard.toast.dockerCheckFailed', { error: e }, 'error');
    }
    return false;
  }
};

const refreshContainers = async (silent = false) => {
  if (!silent) {
    loading.value = true;
    addLogKey('dashboard.toast.refreshing');
  }
  if (!(await checkDocker())) {
    consecutiveFailures += 1;
    containers.value = [];
    if (!silent) {
      loading.value = false;
      addLogKey('dashboard.toast.dockerUnavailable', undefined, 'warn');
    }
    return;
  }
  try {
    const result = await listContainers();
    consecutiveFailures = 0;
    
    // 只有当内容真正改变时才更新，减少 DOM 抖动
    if (JSON.stringify(containers.value) !== JSON.stringify(result)) {
      containers.value = result;
      if (!silent) addLogKey('dashboard.toast.containerUpdated', { count: result.length });
    } else if (!silent) {
      addLogKey('dashboard.toast.containerNoChange');
    }
  } catch (e) {
    consecutiveFailures += 1;
    if (!silent) addLogKey('dashboard.toast.refreshFailed', { error: e }, 'error');
  } finally {
    if (!silent) {
      loading.value = false;
      addLogKey('dashboard.toast.refreshDone');
    }
  }
};

// 单容器操作进行中：容器名 -> 操作类型。用 Record 支持多容器并行操作互不干扰；
// 操作完成后要等容器状态真实刷新（refreshContainers）才复位，避免状态与界面脱节。
const busyContainers = ref<Record<string, 'start' | 'stop'>>({});
const isContainerBusy = (name: string) => Boolean(busyContainers.value[name]);

const startService = async (name: string) => {
  if (busyContainers.value[name]) return; // 防御：正常路径按钮已 disabled
  busyContainers.value = { ...busyContainers.value, [name]: 'start' };
  try {
    addLogKey('dashboard.toast.serviceStarting', { name });
    await startContainer(String(name));
    addLogKey('dashboard.toast.serviceStarted', { name });
    await refreshContainers(true);
  } catch (e) {
    // 失败必须同时给 toast：实时日志只对主动排查的人可见，弹窗才是即时反馈
    addLogKey('dashboard.toast.serviceStartFailed', { error: e }, 'error');
    showToast(t('dashboard.toast.serviceStartFailed', { error: normalizeError(e) }), 'error', 6000);
  } finally {
    const next = { ...busyContainers.value };
    delete next[name];
    busyContainers.value = next;
  }
};

const stopService = async (name: string) => {
  if (busyContainers.value[name]) return;
  busyContainers.value = { ...busyContainers.value, [name]: 'stop' };
  try {
    addLogKey('dashboard.toast.serviceStopping', { name });
    await stopContainer(String(name));
    addLogKey('dashboard.toast.serviceStopped', { name });
    await refreshContainers(true);
  } catch (e) {
    addLogKey('dashboard.toast.serviceStopFailed', { error: e }, 'error');
    showToast(t('dashboard.toast.serviceStopFailed', { error: normalizeError(e) }), 'error', 6000);
  } finally {
    const next = { ...busyContainers.value };
    delete next[name];
    busyContainers.value = next;
  }
};

const openServiceConfig = async (name: string) => {
  try {
    addLogKey('dashboard.toast.configOpening', { name });
    // 容器名统一为 ps-{serviceDir}（如 ps-php82），去掉前缀即得服务配置目录
    const serviceName = String(name).replace(/^ps-/, '');
    await openServiceConfigApi(serviceName);
    addLogKey('dashboard.toast.configOpened', { name: serviceName });
  } catch (e) {
    addLogKey('dashboard.toast.configOpenFailed', { error: normalizeError(e) }, 'error');
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
  addLogKey('dashboard.toast.envStopping');
  
  try {
    await stopEnvironment();
    addLogKey('dashboard.toast.envStopped');
    
    // 等待 1 秒让 Docker API 状态更新
    await new Promise(resolve => setTimeout(resolve, 1000));
    
    await refreshContainers();
  } catch (e: any) {
    addLogKey('dashboard.toast.envStopFailed', { error: e }, 'error');
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
  addLogKey('dashboard.toast.envStarting');
  
  try {
    await startEnvironment();
    addLogKey('dashboard.toast.envStarted');
    
    // 等待 1 秒让 Docker API 状态更新
    await new Promise(resolve => setTimeout(resolve, 1000));
    
    await refreshContainers();
  } catch (e: unknown) {
    const errorMsg = normalizeError(e);
    
    // 检查是否是端口冲突错误
    if (isPortConflictError(errorMsg)) {
      const conflictDetails = stripProtocolPrefix(errorMsg, PORT_CONFLICT_PREFIX);
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
        addLogKey('dashboard.toast.portConflictIgnore', undefined, 'warn');
        try {
          await startEnvironment();
          addLogKey('dashboard.toast.envStarted');
          
          // 等待 1 秒让 Docker API 状态更新
          await new Promise(resolve => setTimeout(resolve, 1000));
          
          await refreshContainers();
        } catch (err) {
          addLogKey('dashboard.toast.envStartFailed', { error: err }, 'error');
        }
      } else {
        // 用户取消
        addLogKey('dashboard.toast.portConflictCancel', undefined, 'warn');
      }
    } else {
      addLogKey('dashboard.toast.envStartFailed', { error: e }, 'error');
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
  addLogKey('dashboard.toast.envRestarting');
  
  try {
    await restartEnvironment();
    addLogKey('dashboard.toast.envRestarted');
    
    // 等待 1 秒让 Docker API 状态更新
    await new Promise(resolve => setTimeout(resolve, 1000));
    
    await refreshContainers();
  } catch (e: any) {
    addLogKey('dashboard.toast.envRestartFailed', { error: e }, 'error');
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
    const existingFiles = await checkConfigFilesExist();
    hasEnvFile.value = existingFiles.some(f => f.includes('.env'));
  } catch (e) {
    console.error('[App] failed to check config file:', e);
    hasEnvFile.value = false;
  }
};

/// 加载工作区状态：配置路径不可用时必须全局可见，否则备份/恢复也会写到错误位置。
async function loadWorkspaceFallbackBanner() {
  try {
    const info = await getWorkspaceInfo();
    if (info?.using_fallback) {
      workspaceFallbackMsg.value = t('workspace.status.fallback', {
        effective: info.effective_path,
        reason: info.fallback_reason
          ? t(info.fallback_reason, { path: info.workspace_path })
          : '',
      });
    } else {
      workspaceFallbackMsg.value = '';
    }

    if (info?.path_missing && !workspaceMissingDismissed.value) {
      workspaceMissingInfo.value = {
        workspace_path: info.workspace_path,
        effective_path: info.effective_path,
      };
      showWorkspaceMissing.value = true;
    } else if (!info?.path_missing) {
      showWorkspaceMissing.value = false;
      workspaceMissingInfo.value = null;
      workspaceMissingDismissed.value = false;
    }
  } catch {
    workspaceFallbackMsg.value = '';
  }
}

async function onWorkspaceMissingResolved() {
  showWorkspaceMissing.value = false;
  workspaceMissingDismissed.value = false;
  await loadWorkspaceFallbackBanner();
  showToast(t('workspace.missing.resolved'), 'success');
}

function onWorkspaceMissingTemp() {
  showWorkspaceMissing.value = false;
  workspaceMissingDismissed.value = true;
  showToast(t('workspace.missing.tempToast'), 'info');
}

function openWorkspaceMissingOrConfig() {
  if (workspaceMissingInfo.value && !showWorkspaceMissing.value) {
    showWorkspaceMissing.value = true;
    return;
  }
  activeTab.value = 'env-config';
}

// 监听 tab 切换，回到 dashboard 时刷新 .env 检测状态
watch(activeTab, async (newTab) => {
  if (newTab === 'dashboard') {
    await checkEnvFileExists();
  }
});

function stopPolling() {
  if (pollTimer !== null) {
    clearTimeout(pollTimer);
    pollTimer = null;
  }
}

// 自适应轮询：正常 5 秒；Docker 连续失败后逐级退避到 15s / 30s，恢复后回到 5 秒。
// 用自调度的 setTimeout 而非 setInterval，间隔才能随失败次数变化。
async function pollOnce() {
  await refreshContainers(true);
  // stopPolling() 可能在 await 期间被调用（组件卸载），此时不再续期
  if (pollTimer === null) return;
  pollTimer = setTimeout(pollOnce, nextPollDelay(consecutiveFailures));
}

function startPolling() {
  stopPolling();
  pollTimer = setTimeout(pollOnce, POLL_INTERVAL_MS);
}

onMounted(async () => {
  // 获取应用版本号
  try {
    const version = await getVersion();
    appVersion.value = `v${version}`;
  } catch (error) {
    console.error('Failed to get app version:', error);
  }
  
  refreshContainers();
  checkEnvFileExists(); // 检查 .env 文件是否存在
  loadWorkspaceFallbackBanner();
  startPolling();
  
  // 监听后端发送的日志事件
  listen('env-log', (event) => {
    const payload = event.payload as unknown;
    if (typeof payload === 'string') {
      addLog(payload, 'info');
      return;
    }
    if (payload && typeof payload === 'object' && 'message' in payload) {
      const { level, message } = payload as { level?: string; message: string };
      const lv = level === 'warn' || level === 'error' ? level : 'info';
      addLog(message, lv);
    }
  });

  // 工作区初始化/切换后刷新，不再整页 reload
  window.addEventListener(WORKSPACE_CHANGED_EVENT, onWorkspaceChanged);

  try {
    const { check } = await import('@tauri-apps/plugin-updater');
    const update = await check();
    if (update) {
      setPendingUpdateVersion(update.version);
    }
  } catch {
    // Dev builds and unsigned local binaries cannot check GitHub latest.json.
  }
});

async function onWorkspaceChanged() {
  await loadWorkspaceFallbackBanner();
  await checkEnvFileExists();
}

onUnmounted(() => {
  stopPolling();
  if (scrollTimeout) clearTimeout(scrollTimeout);
  window.removeEventListener(WORKSPACE_CHANGED_EVENT, onWorkspaceChanged);
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

// 复制面板里当前可见的日志（此前复制的是后端文件日志全文，
// 与界面看到的内容对不上——面板只保留最近 UI_LOG_LIMIT 条）
async function copyLogs() {
  if (logs.value.length === 0) {
    showToast(t('dashboard.log.empty'), 'info');
    return;
  }
  try {
    await writeText(logs.value.map(formatLogLine).join('\n'));
    showToast(t('dashboard.log.copied'), 'success');
  } catch (e) {
    showToast(t('dashboard.log.copyFailed', { error: e }), 'error');
  }
}

// 清空面板（长会话下满屏历史，此前没有出口）
function clearLogPanel() {
  clearLogs();
  showToast(t('dashboard.log.cleared'), 'success');
}

// 导出完整文件日志到用户指定位置（走后端，与面板显示范围无关）
async function exportLogs() {
  try {
    const dest = await save({
      defaultPath: 'php-stack.log',
      filters: [{ name: 'Log', extensions: ['log', 'txt'] }],
    });
    if (!dest) return; // 用户取消
    await exportLogsTo(dest);
    showToast(t('dashboard.log.exported', { path: dest }), 'success');
  } catch (e) {
    showToast(t('dashboard.log.exportFailed', { error: e }), 'error');
  }
}
</script>

<template>
  <div class="flex h-screen w-screen overflow-hidden bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-200 transition-colors duration-300">
    <!-- Sidebar -->
    <div 
      class="bg-white dark:bg-slate-900 flex flex-col border-r border-slate-200 dark:border-slate-800 overflow-y-auto transition-all duration-300 ease-in-out"
      :class="sidebarCollapsed ? 'w-16 sm:w-20 p-2 sm:p-3' : 'w-48 sm:w-52 p-3 sm:p-4'"
    >
      <!-- Logo -->
      <div class="mb-4 sm:mb-6 flex items-center gap-2" :class="sidebarCollapsed ? 'justify-center' : ''">
        <span class="bg-blue-500 text-white p-1 rounded font-bold text-sm sm:text-base">PS</span>
        <span v-if="!sidebarCollapsed" class="text-xl sm:text-2xl font-bold text-blue-600 dark:text-blue-400 hidden sm:inline">PHP-Stack</span>
      </div>
      
      <!-- Menu Items -->
      <div class="flex flex-col gap-2">
        <button
          type="button"
          @click="activeTab = 'dashboard'"
          :class="{ 'active': activeTab === 'dashboard' }" 
          class="sidebar-item text-sm sm:text-base"
          :title="sidebarCollapsed ? $t('sidebar.dashboard') : ''"
        >
          <span class="text-base sm:text-lg">🏠</span>
          <span v-if="!sidebarCollapsed" class="ml-2 hidden sm:inline">{{ $t('sidebar.dashboard') }}</span>
        </button>
        <button
          type="button"
          @click="activeTab = 'env-config'"
          :class="{ 'active': activeTab === 'env-config' }" 
          class="sidebar-item text-sm sm:text-base"
          :title="sidebarCollapsed ? $t('sidebar.envConfig') : ''"
        >
          <span class="text-base sm:text-lg">🛠️</span>
          <span v-if="!sidebarCollapsed" class="ml-2 hidden sm:inline">{{ $t('sidebar.envConfig') }}</span>
        </button>
        <button
          type="button"
          @click="activeTab = 'mirrors-unified'"
          :class="{ 'active': activeTab === 'mirrors-unified' }" 
          class="sidebar-item text-sm sm:text-base"
          :title="sidebarCollapsed ? $t('sidebar.settings') : ''"
        >
          <span class="text-base sm:text-lg">⚙️</span>
          <span v-if="!sidebarCollapsed" class="ml-2 hidden sm:inline">{{ $t('sidebar.settings') }}</span>
        </button>
        <button
          type="button"
          @click="activeTab = 'migration'"
          :class="{ 'active': activeTab === 'migration' }" 
          class="sidebar-item text-sm sm:text-base"
          :title="sidebarCollapsed ? $t('sidebar.migration') : ''"
        >
          <span class="text-base sm:text-lg">📦</span>
          <span v-if="!sidebarCollapsed" class="ml-2 hidden sm:inline">{{ $t('sidebar.migration') }}</span>
        </button>
      </div>
      
      <!-- Version & Toggle Button -->
      <div class="mt-auto pt-3 sm:pt-4 border-t border-slate-200 dark:border-slate-800">
        <button
          type="button"
          @click="activeTab = 'about'"
          :class="{ 'active': activeTab === 'about' }"
          class="sidebar-item text-sm sm:text-base w-full mb-2 sm:mb-3"
          :title="$t('sidebar.about')"
        >
          <span class="text-base sm:text-lg">ℹ️</span>
          <span v-if="!sidebarCollapsed" class="ml-2 hidden sm:inline font-mono">{{ appVersion }}</span>
        </button>
                
        <!-- Toggle Button -->
        <button 
          @click="sidebarCollapsed = !sidebarCollapsed"
          class="w-full py-2 px-3 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors flex items-center justify-center text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200"
          :title="sidebarCollapsed ? $t('sidebar.expand') : $t('sidebar.collapse')"
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
      <!-- 工作区回退全局告警：备份/恢复/启动都走 effective_path，必须跨页可见 -->
      <div
        v-if="workspaceFallbackMsg"
        data-testid="workspace-fallback-banner"
        class="flex-shrink-0 mb-3 sm:mb-4 p-3 sm:p-4 ui-hint-box rounded-xl flex flex-col sm:flex-row items-start sm:items-center gap-3"
      >
        <div class="flex-1 min-w-0">
          <h3 class="font-bold text-sm sm:text-base mb-0.5">{{ $t('workspace.banner.title') }}</h3>
          <p class="text-xs sm:text-sm opacity-90 break-words">{{ workspaceFallbackMsg }}</p>
        </div>
        <button
          type="button"
          @click="openWorkspaceMissingOrConfig"
          class="flex-shrink-0 ui-btn-primary px-3 py-1.5 rounded-lg text-xs sm:text-sm font-bold transition whitespace-nowrap"
        >
          {{ workspaceMissingInfo ? $t('workspace.banner.handle') : $t('workspace.banner.action') }}
        </button>
      </div>

      <div
        v-if="pendingUpdateVersion"
        data-testid="update-available-banner"
        class="flex-shrink-0 mb-3 sm:mb-4 p-3 sm:p-4 ui-hint-box rounded-xl flex flex-col sm:flex-row items-start sm:items-center gap-3"
      >
        <p class="flex-1 min-w-0 text-sm">{{ $t('about.update.banner', { version: pendingUpdateVersion }) }}</p>
        <button
          type="button"
          class="flex-shrink-0 ui-btn-primary px-3 py-1.5 rounded-lg text-xs sm:text-sm font-bold transition"
          @click="activeTab = 'about'"
        >
          {{ $t('about.update.bannerAction') }}
        </button>
      </div>

      <!-- 1. 环境管理 (Dashboard) -->
      <div v-if="activeTab === 'dashboard'" class="flex-1 flex flex-col overflow-hidden">
        <header class="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-4 mb-6 sm:mb-8">
          <h1 class="text-3xl font-bold">{{ $t('dashboard.title') }}</h1>
          <div class="flex flex-col sm:flex-row gap-3 w-full sm:w-auto">
            <button 
              @click="() => refreshContainers()" 
              :disabled="loading"
              class="w-full sm:w-auto ui-btn-emphasis disabled:opacity-50 px-4 py-2 rounded-lg font-medium transition"
            >
              {{ loading ? $t('dashboard.refreshing') : $t('dashboard.refresh') }}
            </button>
            <div class="flex flex-col sm:flex-row gap-3 w-full sm:w-auto">
              <button 
                @click="handleStartEnvironment"
                :disabled="!canStart || starting"
                class="w-full sm:w-auto ui-btn-primary disabled:opacity-50 disabled:cursor-not-allowed px-4 py-2 rounded-lg font-medium transition flex items-center justify-center gap-2"
                :title="!hasEnvFile ? $t('dashboard.startTooltip.noEnv') : (!canStart ? $t('dashboard.startTooltip.hasRunning') : '')"
              >
                <svg v-if="operationType === 'start'" class="animate-spin" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
                <svg v-else xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>
                {{ operationType === 'start' ? $t('dashboard.startingEnv') : $t('dashboard.startEnv') }}
              </button>
              <button 
                @click="handleRestartEnvironment"
                :disabled="!canRestart || starting"
                class="w-full sm:w-auto ui-btn-emphasis disabled:opacity-50 disabled:cursor-not-allowed px-4 py-2 rounded-lg font-medium transition flex items-center justify-center gap-2"
                :title="!canRestart ? $t('dashboard.restartTooltip') : ''"
              >
                <svg v-if="operationType === 'restart'" class="animate-spin" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
                <svg v-else xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"></polyline><polyline points="1 20 1 14 7 14"></polyline><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path></svg>
                {{ operationType === 'restart' ? $t('dashboard.restartingEnv') : $t('dashboard.restartEnv') }}
              </button>
              <button 
                @click="handleStopEnvironment"
                :disabled="!canStop || starting"
                class="w-full sm:w-auto ui-btn-danger disabled:opacity-50 disabled:cursor-not-allowed px-4 py-2 rounded-lg font-medium transition flex items-center justify-center gap-2"
                :title="!canStop ? $t('dashboard.stopTooltip') : ''"
              >
                <svg v-if="operationType === 'stop'" class="animate-spin" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
                <svg v-else xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="6" y="6" width="12" height="12"></rect></svg>
                {{ operationType === 'stop' ? $t('dashboard.stoppingEnv') : $t('dashboard.stopEnv') }}
              </button>
            </div>
          </div>
        </header>

        <!-- Docker Error Alert -->
        <div v-if="dockerError" class="mb-8 p-6 bg-rose-500/10 border border-rose-500/20 rounded-2xl flex items-center gap-4 text-rose-600 dark:text-rose-400">
          <div class="p-3 bg-rose-500/20 rounded-full text-rose-500">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
          </div>
          <div class="flex-1">
            <h3 class="font-bold text-lg mb-1 text-rose-600 dark:text-rose-400">{{ $t('dashboard.dockerError.title') }}</h3>
            <p class="text-sm opacity-90">{{ dockerError }}</p>
          </div>
          <button 
            @click="() => refreshContainers()"
            class="ui-btn-danger px-4 py-2 rounded-lg transition font-bold text-sm"
          >
            {{ $t('dashboard.dockerError.retry') }}
          </button>
        </div>

        <!-- No Env File Alert -->
        <div v-if="!hasEnvFile" class="mb-8 p-6 ui-hint-box rounded-2xl flex items-center gap-4">
          <div class="p-3 bg-blue-500/20 rounded-full text-blue-500">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
          </div>
          <div class="flex-1">
            <h3 class="font-bold text-lg mb-1 text-blue-600 dark:text-blue-300">{{ $t('dashboard.noEnvFile.title') }}</h3>
            <p class="text-sm opacity-90">{{ $t('dashboard.noEnvFile.description') }}</p>
          </div>
          <button 
            @click="activeTab = 'env-config'"
            class="ui-btn-primary px-4 py-2 rounded-lg transition font-bold text-sm whitespace-nowrap"
          >
            {{ $t('dashboard.noEnvFile.action') }}
          </button>
        </div>

        <!-- Container Grid -->
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4 sm:gap-6 overflow-y-auto mb-8 pr-2">
          <div v-for="c in containers" :key="String(c.id)" class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-5 hover:border-blue-500/50 transition-colors shadow-lg">
            <div class="flex justify-between items-start mb-4">
              <span class="text-slate-500 dark:text-slate-400 text-xs font-mono uppercase tracking-wider">{{ String(c.image).split(':')[0] }}</span>
              <span 
                :class="isContainerRunning(c.state) ? 'text-emerald-400' : 'text-rose-400'"
                class="flex items-center gap-1.5 text-xs font-bold uppercase tracking-tighter"
              >
                <span :class="[isContainerRunning(c.state) ? 'bg-emerald-500' : 'bg-rose-500', { 'animate-pulse': isContainerRunning(c.state) }]" class="w-2 h-2 rounded-full"></span>
                {{ isContainerRunning(c.state) ? $t('dashboard.container.running') : $t('dashboard.container.stopped') }}
              </span>
            </div>
            <div class="text-xl font-bold mb-1 truncate text-slate-900 dark:text-slate-200" :title="String(c.name)">{{ String(c.name) }}</div>
            <div class="text-slate-500 dark:text-slate-500 text-xs mb-4">
              <span v-if="c.ports.length > 0">Ports: {{ c.ports.join(', ') }}</span>
              <span v-else>{{ $t('dashboard.container.noPorts') }}</span>
            </div>
            
            <div class="flex gap-2">
              <button
                v-if="!isContainerRunning(c.state)"
                @click="startService(String(c.name))"
                :disabled="isContainerBusy(String(c.name))"
                class="flex-1 py-2 ui-btn-soft rounded text-sm font-medium transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-1.5"
              >
                <svg v-if="isContainerBusy(String(c.name))" class="animate-spin shrink-0" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
                {{ $t('dashboard.container.start') }}
              </button>
              <button
                v-else
                @click="stopService(String(c.name))"
                :disabled="isContainerBusy(String(c.name))"
                class="flex-1 py-2 bg-rose-600/20 hover:bg-rose-600 text-rose-600 dark:text-rose-400 hover:text-white border border-rose-600/30 rounded text-sm font-medium transition-all disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-rose-600/20 flex items-center justify-center gap-1.5"
              >
                <svg v-if="isContainerBusy(String(c.name))" class="animate-spin shrink-0" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
                {{ $t('dashboard.container.stop') }}
              </button>
              <button 
                @click="openServiceConfig(String(c.name))"
                class="px-3 py-2 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded text-sm transition border border-slate-300 dark:border-slate-700 text-slate-700 dark:text-slate-300"
              >
                {{ $t('dashboard.container.config') }}
              </button>
            </div>
          </div>

          <!-- Empty State -->
          <div v-if="containers.length === 0 && !loading" class="col-span-full py-20 text-center bg-slate-100/50 dark:bg-slate-900/50 border-2 border-dashed border-slate-300 dark:border-slate-800 rounded-2xl">
            <div class="text-slate-500 dark:text-slate-500 mb-2">{{ t('dashboard.empty.title') }}</div>
            <div class="text-slate-600 dark:text-slate-600 text-sm">{{ t('dashboard.empty.description', { page: t('sidebar.envConfig') }) }}</div>
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

      <div v-if="activeTab === 'about'" class="flex-1 flex flex-col overflow-hidden">
        <AboutPage />
      </div>

      <!-- Log Panel (Global) -->
      <div class="mt-auto border-t border-slate-200 dark:border-slate-800 pt-3 sm:pt-4 bg-slate-50/50 dark:bg-slate-950/50 backdrop-blur-md">
        <div class="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-2 sm:gap-3 mb-2 sm:mb-3">
          <h2 class="text-lg font-bold flex items-center gap-2 text-slate-600 dark:text-slate-400">
            <span class="w-2 h-2 bg-blue-500 rounded-full" :class="{ 'animate-pulse': loading }"></span> 
            {{ $t('dashboard.log.title') }}
          </h2>
          <div class="flex flex-wrap gap-2">
            <button 
              @click="copyLogs"
              class="text-xs px-2 py-1 bg-slate-200 dark:bg-slate-800 hover:bg-slate-300 dark:hover:bg-slate-700 rounded text-slate-600 dark:text-slate-400 hover:text-blue-600 dark:hover:text-blue-400 transition-colors flex items-center gap-1"
              :title="$t('dashboard.log.copyTip')"
            >
              {{ $t('dashboard.log.copy') }}
            </button>
            <button 
              @click="clearLogPanel"
              class="text-xs px-2 py-1 bg-slate-200 dark:bg-slate-800 hover:bg-slate-300 dark:hover:bg-slate-700 rounded text-slate-600 dark:text-slate-400 hover:text-blue-600 dark:hover:text-blue-400 transition-colors flex items-center gap-1"
              :title="$t('dashboard.log.clearTip')"
            >
              {{ $t('dashboard.log.clear') }}
            </button>
            <button 
              @click="exportLogs"
              class="text-xs px-2 py-1 bg-slate-200 dark:bg-slate-800 hover:bg-slate-300 dark:hover:bg-slate-700 rounded text-slate-600 dark:text-slate-400 hover:text-blue-600 dark:hover:text-blue-400 transition-colors flex items-center gap-1"
              :title="$t('dashboard.log.exportTip')"
            >
              {{ $t('dashboard.log.export') }}
            </button>
            <button 
              @click="scrollToBottom"
              class="text-xs px-2 py-1 bg-slate-200 dark:bg-slate-800 hover:bg-slate-300 dark:hover:bg-slate-700 rounded text-slate-600 dark:text-slate-400 hover:text-blue-600 dark:hover:text-blue-400 transition-colors flex items-center gap-1"
              title=""
            >
              {{ $t('dashboard.log.bottom') }}
            </button>
            <button 
              @click="showLogs = !showLogs"
              class="text-xs px-2 py-1 bg-slate-200 dark:bg-slate-800 hover:bg-slate-300 dark:hover:bg-slate-700 rounded text-slate-600 dark:text-slate-400 hover:text-blue-600 dark:hover:text-blue-400 transition-colors flex items-center gap-1"
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
            class="bg-slate-100 dark:bg-black/40 p-3 sm:p-4 rounded-xl font-mono text-xs sm:text-sm border border-slate-300 dark:border-slate-800 h-32 sm:h-40 overflow-y-auto scrollbar-hide shadow-inner overflow-hidden"
          >
            <div
              v-for="(log, i) in logs"
              :key="i"
              class="mb-1 last:mb-0 animate-in fade-in slide-in-from-left-2 duration-300"
              :class="{
                'text-slate-600 dark:text-slate-300': log.level === 'info',
                'text-amber-600 dark:text-amber-400': log.level === 'warn',
                'text-rose-600 dark:text-rose-400': log.level === 'error',
              }"
            >
              {{ formatLogLine(log) }}
            </div>
            <div v-if="logs.length === 0" class="text-slate-500 dark:text-slate-600 italic">{{ $t('dashboard.log.empty') }}</div>
          </div>
        </transition>
        <p v-if="showLogs" class="mt-1.5 text-[10px] sm:text-xs text-slate-500 dark:text-slate-500">
          {{ $t('dashboard.log.panelHint', { limit: UI_LOG_LIMIT, action: $t('dashboard.log.export') }) }}
        </p>
      </div>
    </div>

    <!-- Global Toast -->
    <Toast />
    
    <!-- Global Confirm Dialog -->
    <ConfirmDialog />
    
    <!-- Workspace Initialization Dialog -->
    <WorkspaceInitDialog />

    <!-- 配置工作区目录不存在：重建 / 选新路径 / 临时回退 -->
    <WorkspaceMissingDialog
      :open="showWorkspaceMissing"
      :info="workspaceMissingInfo"
      @resolved="onWorkspaceMissingResolved"
      @dismiss-temp="onWorkspaceMissingTemp"
    />

    <!-- Start Environment Confirmation Dialog -->
    <div v-if="showStartConfirm" class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50">
      <div class="bg-white dark:bg-slate-900 border border-slate-300 dark:border-slate-700 rounded-xl p-8 max-w-md w-full shadow-2xl">
        <h2 class="text-2xl font-bold text-slate-900 dark:text-white mb-4">{{ $t('dashboard.startConfirm.title') }}</h2>
        <p class="text-slate-600 dark:text-slate-400 mb-6">
          {{ $t('dashboard.startConfirm.message', { proxy: '' }) }}
          <strong>{{ $t('dashboard.startConfirm.proxy') }}</strong>
        </p>

        <div class="space-y-4">
          <div class="flex gap-3">
            <button 
              @click="showStartConfirm = false"
              class="flex-1 px-4 py-2 ui-btn-secondary rounded-lg font-medium transition"
            >
              {{ $t('common.cancel') }}
            </button>
            <button 
              @click="goToMirrorSettings"
              class="flex-1 ui-btn-primary px-4 py-2 rounded-lg font-medium transition"
            >
              {{ $t('dashboard.startConfirm.goMirror') }}
            </button>
          </div>
          <button 
            @click="confirmStart"
            class="w-full ui-btn-primary px-6 py-2 rounded-lg font-bold transition shadow-lg shadow-blue-600/20"
          >
            {{ $t('dashboard.startConfirm.directStart') }}
          </button>
        </div>
      </div>
    </div>

    <!-- Restart Environment Confirmation Dialog -->
    <div v-if="showRestartConfirm" class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50">
      <div class="bg-white dark:bg-slate-900 border border-slate-300 dark:border-slate-700 rounded-xl p-8 max-w-md w-full shadow-2xl">
        <h2 class="text-2xl font-bold text-slate-900 dark:text-white mb-4">{{ $t('dashboard.restartConfirm.title') }}</h2>
        <p class="text-slate-600 dark:text-slate-400 mb-6">
          {{ $t('dashboard.restartConfirm.message', { warning: '' }) }}
          <strong class="text-blue-600 dark:text-blue-400">{{ $t('dashboard.restartConfirm.warning') }}</strong>
        </p>
        <div class="space-y-4">
          <button 
            @click="showRestartConfirm = false"
            class="w-full px-4 py-2 ui-btn-secondary rounded-lg font-medium transition"
          >
            {{ $t('common.cancel') }}
          </button>
          <button 
            @click="confirmRestart"
            class="w-full ui-btn-emphasis px-6 py-2 rounded-lg font-bold transition shadow-lg shadow-slate-900/20"
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
  @apply w-full text-left px-4 py-3 rounded-lg transition-all cursor-pointer text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800 hover:text-slate-900 dark:hover:text-slate-100 border border-transparent flex items-center bg-transparent;
}
.sidebar-item.active {
  @apply bg-blue-600/10 text-blue-600 dark:text-blue-400 border-blue-600/20;
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
