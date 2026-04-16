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

onMounted(() => {
  loadVersions();
  loadInstalled();
});
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden">
    <header class="mb-8">
      <h1 class="text-3xl font-bold mb-2">软件管理中心</h1>
      <p class="text-slate-400">一键安装和管理开发环境软件</p>
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
