<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';

interface SoftwareVersion {
  version: string;
  image_tag: string;
  description: string;
  is_stable: boolean;
}

interface InstalledSoftware {
  id: string;
  name: string;
  spec: {
    software_type: string;
    version: string;
  };
  status: string;
  created_at: string;
}

type SoftwareType = 'php' | 'mysql' | 'redis' | 'nginx' | 'mongodb';

const selectedType = ref<SoftwareType>('php');
const availableVersions = ref<SoftwareVersion[]>([]);
const installedList = ref<InstalledSoftware[]>([]);
const loading = ref(false);
const installing = ref(false);
const installLog = ref<string[]>([]);
const showInstallModal = ref(false);
const selectedVersion = ref<SoftwareVersion | null>(null);
const customPort = ref<number | null>(null);

// Phase 4: 影响评估和智能重启
interface RestartImpact {
  services_to_restart: string[];
  dependency_chain: string[];
  total_affected: number;
}

const showImpactDialog = ref(false);
const restartImpact = ref<RestartImpact | null>(null);
const restartingService = ref<string>('');
const isAnalyzing = ref(false);
const isRestarting = ref(false);

// Phase 4: Compose 文件查看器
const showComposeViewer = ref(false);
const composeContent = ref<string>('');
const isLoadingCompose = ref(false);

// 软件类型配置（softwareTypeKey -> 后端期望的 Enum 名称）
const softwareTypeMap: Record<SoftwareType, string> = {
  php: 'PHP',
  mysql: 'MySQL',
  redis: 'Redis',
  nginx: 'Nginx',
  mongodb: 'MongoDB',
};

const softwareTypes = [
  { key: 'php' as SoftwareType, label: 'PHP', icon: '🐘', color: 'blue' },
  { key: 'mysql' as SoftwareType, label: 'MySQL', icon: '🐬', color: 'orange' },
  { key: 'redis' as SoftwareType, label: 'Redis', icon: '🔴', color: 'red' },
  { key: 'nginx' as SoftwareType, label: 'Nginx', icon: '🚀', color: 'green' },
  { key: 'mongodb' as SoftwareType, label: 'MongoDB', icon: '🍃', color: 'emerald' },
];

// 加载可用版本
const loadVersions = async () => {
  try {
    loading.value = true;
    const versions = await invoke('get_available_versions', {
      softwareTypeStr: selectedType.value,
    }) as SoftwareVersion[];
    availableVersions.value = versions;
  } catch (e) {
    console.error('加载版本失败:', e);
  } finally {
    loading.value = false;
  }
};

// 加载已安装列表
const loadInstalled = async () => {
  try {
    const list = await invoke('list_installed_software') as InstalledSoftware[];
    installedList.value = list;
  } catch (e) {
    console.error('加载已安装列表失败:', e);
  }
};

// 打开安装对话框
const openInstallModal = (version: SoftwareVersion) => {
  selectedVersion.value = version;
  customPort.value = null;
  showInstallModal.value = true;
};

// 执行安装
const handleInstall = async () => {
  if (!selectedVersion.value) return;

  try {
    installing.value = true;
    installLog.value = ['开始安装...'];

    // 自动分配端口
    const portMappings = await invoke('allocate_ports', {
      softwareTypeStr: selectedType.value,
      preferredPorts: customPort.value ? [customPort.value] : [],
    }) as Record<number, number>;

    installLog.value.push(`端口分配: ${JSON.stringify(portMappings)}`);

    // 构建安装规格
    const spec = {
      software_type: softwareTypeMap[selectedType.value], // 使用映射表获取正确的枚举名称
      version: selectedVersion.value.version,
      custom_image: null,
      port_mappings: portMappings,
      volume_path: null,
      env_vars: {},
      extra_args: [],
    };

    installLog.value.push(`正在拉取镜像并创建容器...`);
    const containerName = await invoke('install_software', { spec }) as string;
    
    installLog.value.push(`✅ 安装成功！容器名称: ${containerName}`);
    
    // 延迟关闭对话框，让用户看到日志
    setTimeout(() => {
      showInstallModal.value = false;
      installing.value = false;
      installLog.value = [];
      loadInstalled();
    }, 2000);
  } catch (e) {
    installLog.value.push(`❌ 安装失败: ${e}`);
    installing.value = false;
  }
};

// 卸载软件
const handleUninstall = async (name: string) => {
  if (!confirm(`确定要卸载 ${name} 吗？此操作不可恢复！`)) return;

  try {
    await invoke('uninstall_software', { name });
    await loadInstalled();
  } catch (e) {
    alert(`卸载失败: ${e}`);
  }
};

// Phase 4: 分析重启影响
const analyzeRestartImpact = async (containerName: string) => {
  try {
    isAnalyzing.value = true;
    const impact = await invoke('analyze_restart_impact', { 
      serviceName: containerName 
    }) as RestartImpact;
    
    restartImpact.value = impact;
    restartingService.value = containerName;
    showImpactDialog.value = true;
  } catch (e) {
    console.error('分析失败:', e);
    alert(`分析失败: ${e}`);
  } finally {
    isAnalyzing.value = false;
  }
};

// Phase 4: 执行智能重启
const executeSmartRestart = async () => {
  try {
    isRestarting.value = true;
    const impact = await invoke('smart_restart_service', { 
      serviceName: restartingService.value 
    }) as RestartImpact;
    
    // 显示成功提示
    alert(`✅ 智能重启完成！\n已重启 ${impact.total_affected} 个服务：\n${impact.services_to_restart.join(', ')}`);
    
    showImpactDialog.value = false;
    await loadInstalled();
  } catch (e) {
    console.error('重启失败:', e);
    alert(`重启失败: ${e}`);
  } finally {
    isRestarting.value = false;
  }
};

// Phase 4: 查看 docker-compose.yml
const viewComposeFile = async () => {
  try {
    isLoadingCompose.value = true;
    const content = await invoke('read_compose_file') as string;
    composeContent.value = content;
    showComposeViewer.value = true;
  } catch (e) {
    console.error('读取失败:', e);
    alert(`读取失败: ${e}`);
  } finally {
    isLoadingCompose.value = false;
  }
};

// 判断是否已安装某个版本
const isInstalled = (version: string): boolean => {
  return installedList.value.some(
    (item) =>
      item.spec.software_type.toLowerCase() === selectedType.value &&
      item.spec.version === version
  );
};

// 获取已安装的实例
const getInstalledInstance = (version: string): InstalledSoftware | undefined => {
  return installedList.value.find(
    (item) =>
      item.spec.software_type.toLowerCase() === selectedType.value &&
      item.spec.version === version
  );
};

// Phase 4: 获取服务的依赖关系
const getServiceDependencies = (softwareType: SoftwareType): string[] => {
  const deps: Record<SoftwareType, string[]> = {
    php: ['MySQL', 'Redis'],
    mysql: [],
    redis: [],
    nginx: ['PHP'],
    mongodb: [],
  };
  return deps[softwareType];
};

onMounted(() => {
  loadVersions();
  loadInstalled();
});
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden">
    <header class="mb-8">
      <div class="flex justify-between items-start">
        <div>
          <h1 class="text-3xl font-bold mb-2">软件管理中心</h1>
          <p class="text-slate-400">一键安装和管理开发环境软件</p>
        </div>
        
        <!-- Phase 4: Compose 文件查看按钮 -->
        <button
          @click="viewComposeFile"
          :disabled="isLoadingCompose"
          class="px-4 py-2 bg-indigo-600/20 hover:bg-indigo-600 text-indigo-400 hover:text-white border border-indigo-600/30 rounded-lg text-sm font-medium transition-all disabled:opacity-50 flex items-center gap-2"
        >
          <span v-if="isLoadingCompose" class="inline-block animate-spin rounded-full h-4 w-4 border-b-2 border-indigo-400"></span>
          📄 {{ isLoadingCompose ? '加载中...' : '查看 Compose' }}
        </button>
      </div>
    </header>

    <!-- 软件类型选择 -->
    <div class="flex gap-3 mb-6 overflow-x-auto pb-2 software-type-tabs">
      <button
        v-for="type in softwareTypes"
        :key="type.key"
        @click="selectedType = type.key as SoftwareType; loadVersions()"
        :class="[
          'px-6 py-3 rounded-xl font-medium transition-all border flex items-center gap-2 whitespace-nowrap',
          selectedType === type.key
            ? `bg-${type.color}-600/20 border-${type.color}-500/50 text-${type.color}-400`
            : 'bg-slate-900 border-slate-800 text-slate-400 hover:border-slate-700',
        ]"
      >
        <span class="text-xl">{{ type.icon }}</span>
        {{ type.label }}
      </button>
    </div>

    <!-- 版本列表 -->
    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 overflow-y-auto pr-2 max-h-[calc(100vh-280px)]">
      <div
        v-for="version in availableVersions"
        :key="version.version"
        class="bg-slate-900 border border-slate-800 rounded-xl p-5 hover:border-blue-500/30 transition-all"
      >
        <div class="flex justify-between items-start mb-3">
          <div>
            <h3 class="text-xl font-bold">{{ version.version }}</h3>
            <p class="text-sm text-slate-500 mt-1">{{ version.description }}</p>
            
            <!-- Phase 4: 依赖关系图标 -->
            <div v-if="getServiceDependencies(selectedType).length > 0" class="mt-2 flex items-center gap-1">
              <span class="text-xs text-slate-500">依赖:</span>
              <span v-for="dep in getServiceDependencies(selectedType)" :key="dep"
                    class="px-1.5 py-0.5 bg-slate-800 text-slate-400 text-xs rounded border border-slate-700">
                {{ dep }}
              </span>
            </div>
          </div>
          <span
            v-if="version.is_stable"
            class="px-2 py-1 bg-emerald-500/10 text-emerald-400 text-xs rounded-md border border-emerald-500/20"
          >
            Stable
          </span>
        </div>

        <!-- 已安装状态 -->
        <div v-if="isInstalled(version.version)" class="mt-4">
          <div class="p-3 bg-blue-500/10 border border-blue-500/20 rounded-lg mb-3">
            <div class="text-sm text-blue-400 font-medium">✓ 已安装</div>
            <div class="text-xs text-slate-500 mt-1">
              状态: {{ getInstalledInstance(version.version)?.status || 'Unknown' }}
            </div>
          </div>
          
          <!-- Phase 4: 智能重启按钮 -->
          <button
            @click="analyzeRestartImpact(getInstalledInstance(version.version)!.name)"
            :disabled="isAnalyzing"
            class="w-full py-2 mb-2 bg-purple-600/20 hover:bg-purple-600 text-purple-400 hover:text-white border border-purple-600/30 rounded-lg text-sm font-medium transition-all disabled:opacity-50 flex items-center justify-center gap-2"
          >
            <span v-if="isAnalyzing && restartingService === getInstalledInstance(version.version)!.name" class="inline-block animate-spin rounded-full h-4 w-4 border-b-2 border-purple-400"></span>
            {{ isAnalyzing && restartingService === getInstalledInstance(version.version)!.name ? '分析中...' : '🔄 智能重启' }}
          </button>
          
          <button
            @click="handleUninstall(getInstalledInstance(version.version)!.name)"
            class="w-full py-2 bg-rose-600/20 hover:bg-rose-600 text-rose-400 hover:text-white border border-rose-600/30 rounded-lg text-sm font-medium transition-all"
          >
            卸载
          </button>
        </div>

        <!-- 未安装状态 -->
        <button
          v-else
          @click="openInstallModal(version)"
          class="w-full mt-4 py-2.5 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition-all"
        >
          安装
        </button>
      </div>

      <!-- 加载中 -->
      <div v-if="loading" class="col-span-full py-12 text-center">
        <div class="inline-block animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
        <p class="text-slate-500 mt-3">加载版本信息...</p>
      </div>

      <!-- 空状态 -->
      <div
        v-if="!loading && availableVersions.length === 0"
        class="col-span-full py-12 text-center bg-slate-900/50 border-2 border-dashed border-slate-800 rounded-2xl"
      >
        <p class="text-slate-500">暂无可用版本</p>
      </div>
    </div>

    <!-- 安装对话框 -->
    <div
      v-if="showInstallModal"
      class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50"
      @click.self="showInstallModal = false"
    >
      <div class="bg-slate-900 border border-slate-800 rounded-2xl p-6 max-w-md w-full mx-4 shadow-2xl">
        <h3 class="text-xl font-bold mb-4">
          安装 {{ softwareTypes.find(t => t.key === selectedType)?.label }} {{ selectedVersion?.version }}
        </h3>

        <div class="space-y-4 mb-6">
          <div class="p-4 bg-slate-800/50 rounded-lg border border-slate-700">
            <div class="text-sm text-slate-400 mb-2">镜像标签</div>
            <div class="font-mono text-blue-300">{{ selectedVersion?.image_tag }}</div>
          </div>

          <div>
            <label class="block text-sm text-slate-400 mb-2">自定义端口（可选）</label>
            <input
              v-model.number="customPort"
              type="number"
              placeholder="留空则自动分配"
              class="w-full bg-slate-800 border border-slate-700 rounded-lg px-4 py-2 focus:ring-2 focus:ring-blue-500 outline-none"
            />
            <p class="text-xs text-slate-500 mt-1">系统将自动检测端口冲突</p>
          </div>

          <!-- 安装日志 -->
          <div v-if="installLog.length > 0" class="bg-black/40 p-3 rounded-lg font-mono text-xs text-blue-300/80 max-h-32 overflow-y-auto">
            <div v-for="(log, i) in installLog" :key="i" class="mb-1">{{ log }}</div>
          </div>
        </div>

        <div class="flex gap-3">
          <button
            @click="showInstallModal = false"
            :disabled="installing"
            class="flex-1 py-2.5 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg font-medium transition disabled:opacity-50"
          >
            取消
          </button>
          <button
            @click="handleInstall"
            :disabled="installing"
            class="flex-1 py-2.5 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition disabled:opacity-50 flex items-center justify-center gap-2"
          >
            <span v-if="installing" class="inline-block animate-spin rounded-full h-4 w-4 border-b-2 border-white"></span>
            {{ installing ? '安装中...' : '确认安装' }}
          </button>
        </div>
      </div>
    </div>

    <!-- Phase 4: 影响评估对话框 -->
    <div
      v-if="showImpactDialog"
      class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50"
      @click.self="showImpactDialog = false"
    >
      <div class="bg-slate-900 border border-slate-800 rounded-2xl p-6 max-w-lg w-full mx-4 shadow-2xl">
        <h3 class="text-xl font-bold mb-4 flex items-center gap-2">
          <span class="text-purple-400">🔄</span>
          智能重启影响评估
        </h3>

        <div class="space-y-4 mb-6">
          <!-- 目标服务 -->
          <div class="p-4 bg-purple-500/10 border border-purple-500/20 rounded-lg">
            <div class="text-sm text-purple-400 mb-1">目标服务</div>
            <div class="font-mono text-white">{{ restartingService }}</div>
          </div>

          <!-- 影响范围 -->
          <div v-if="restartImpact" class="space-y-3">
            <div class="text-sm text-slate-400">
              将影响 <span class="text-yellow-400 font-bold">{{ restartImpact.total_affected }}</span> 个服务
            </div>

            <!-- 依赖链 -->
            <div class="bg-black/40 p-3 rounded-lg font-mono text-xs space-y-1">
              <div v-for="(chain, i) in restartImpact.dependency_chain" :key="i" 
                   :class="i === 0 ? 'text-purple-300' : 'text-blue-300/80'">
                {{ chain }}
              </div>
            </div>

            <!-- 需要重启的服务列表 -->
            <div class="p-3 bg-slate-800/50 rounded-lg border border-slate-700">
              <div class="text-sm text-slate-400 mb-2">需要重启的服务：</div>
              <div class="flex flex-wrap gap-2">
                <span v-for="service in restartImpact.services_to_restart" :key="service"
                      class="px-2 py-1 bg-blue-500/20 text-blue-300 text-xs rounded border border-blue-500/30">
                  {{ service }}
                </span>
              </div>
            </div>
          </div>

          <!-- 加载中 -->
          <div v-if="isAnalyzing" class="py-8 text-center">
            <div class="inline-block animate-spin rounded-full h-8 w-8 border-b-2 border-purple-500"></div>
            <p class="text-slate-500 mt-3">正在分析依赖关系...</p>
          </div>
        </div>

        <div class="flex gap-3">
          <button
            @click="showImpactDialog = false"
            :disabled="isRestarting"
            class="flex-1 py-2.5 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg font-medium transition disabled:opacity-50"
          >
            取消
          </button>
          <button
            @click="executeSmartRestart"
            :disabled="isRestarting || isAnalyzing"
            class="flex-1 py-2.5 bg-purple-600 hover:bg-purple-700 text-white rounded-lg font-medium transition disabled:opacity-50 flex items-center justify-center gap-2"
          >
            <span v-if="isRestarting" class="inline-block animate-spin rounded-full h-4 w-4 border-b-2 border-white"></span>
            {{ isRestarting ? '重启中...' : '确认重启' }}
          </button>
        </div>
      </div>
    </div>

    <!-- Phase 4: Compose 文件查看器 -->
    <div
      v-if="showComposeViewer"
      class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50"
      @click.self="showComposeViewer = false"
    >
      <div class="bg-slate-900 border border-slate-800 rounded-2xl p-6 max-w-4xl w-full mx-4 shadow-2xl max-h-[90vh] flex flex-col">
        <div class="flex justify-between items-center mb-4">
          <h3 class="text-xl font-bold flex items-center gap-2">
            <span class="text-indigo-400">📄</span>
            docker-compose.yml
          </h3>
          <button
            @click="showComposeViewer = false"
            class="text-slate-400 hover:text-white transition"
          >
            ✕
          </button>
        </div>

        <div class="flex-1 overflow-y-auto bg-black/40 p-4 rounded-lg font-mono text-xs text-green-300/90 border border-slate-700">
          <pre class="whitespace-pre-wrap">{{ composeContent }}</pre>
        </div>

        <div class="mt-4 flex gap-3">
          <button
            @click="showComposeViewer = false"
            class="flex-1 py-2.5 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg font-medium transition"
          >
            关闭
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
@reference "tailwindcss";

/* 自定义滚动条样式 - 垂直滚动 */
.overflow-y-auto::-webkit-scrollbar {
  width: 8px;
}

.overflow-y-auto::-webkit-scrollbar-track {
  background: rgba(30, 41, 59, 0.5);
  border-radius: 4px;
}

.overflow-y-auto::-webkit-scrollbar-thumb {
  background: rgba(71, 85, 105, 0.8);
  border-radius: 4px;
}

.overflow-y-auto::-webkit-scrollbar-thumb:hover {
  background: rgba(100, 116, 139, 1);
}

/* 自定义滚动条样式 - 横向滚动（软件类型标签） */
.software-type-tabs::-webkit-scrollbar {
  height: 6px;
}

.software-type-tabs::-webkit-scrollbar-track {
  background: rgba(30, 41, 59, 0.3);
  border-radius: 3px;
}

.software-type-tabs::-webkit-scrollbar-thumb {
  background: rgba(71, 85, 105, 0.6);
  border-radius: 3px;
}

.software-type-tabs::-webkit-scrollbar-thumb:hover {
  background: rgba(100, 116, 139, 0.9);
}
</style>
