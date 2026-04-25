<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { ServiceEntry, EnvConfig, VersionInfo } from '../types/env-config';
import { showToast } from '../composables/useToast';
import { showConfirm } from '../composables/useConfirmDialog';

const emit = defineEmits<{
  (e: 'request-switch-tab', tabName: string): void;
}>();

// Available versions (将从后端动态加载)
const phpVersions = ref<VersionInfo[]>([]);
const mysqlVersions = ref<VersionInfo[]>([]);
const redisVersions = ref<VersionInfo[]>([]);
const nginxVersions = ref<VersionInfo[]>([]);

const commonExtensions = [
  'pdo_mysql', 'mysqli', 'mbstring', 'gd', 'curl', 'opcache', 'bcmath',
  'redis', 'xdebug', 'swoole', 'zip', 'pcntl', 'sockets', 'intl',
  'soap', 'imagick', 'mongodb', 'amqp', 'memcached',
];

// State
const phpServices = ref<ServiceEntry[]>([]);
const mysqlServices = ref<ServiceEntry[]>([]);
const redisServices = ref<ServiceEntry[]>([]);
const nginxServices = ref<ServiceEntry[]>([]);

const sourceDir = ref('./www');
const timezone = ref('Asia/Shanghai');
const mysqlRootPassword = ref('root');  // MySQL root密码
const workspacePath = ref<string>('加载中...');

const loading = ref(false);
const applying = ref(false);
const starting = ref(false);
const previewEnv = ref('');
const previewCompose = ref('');
const showPreviewModal = ref(false);
const hasEnvFile = ref(false);  // .env 文件是否存在

// Nginx 配置提示状态
const showNginxHint = ref(false);
const phpContainerNames = ref<string[]>([]);
const nginxServicesList = ref<Array<{ name: string; version: string; port?: number }>>([]); // 存储所有 Nginx 服务信息
const showStartConfirm = ref(false);

// Load existing config on mount
onMounted(async () => {
  await loadWorkspaceInfo();
  await loadVersionMappings();
  await checkEnvFileExists();
  await loadExistingConfig();
});

async function loadWorkspaceInfo() {
  try {
    const info = await invoke<any>('get_workspace_info');
    if (info) {
      workspacePath.value = info.workspace_path;
    } else {
      workspacePath.value = '未配置';
    }
  } catch (e) {
    workspacePath.value = '获取失败';
  }
}

// 检查 .env 文件是否存在
async function checkEnvFileExists() {
  try {
    const existingFiles = await invoke<string[]>('check_config_files_exist');
    hasEnvFile.value = existingFiles.some(f => f.includes('.env'));
    console.log('[EnvConfig] .env 文件存在:', hasEnvFile.value);
  } catch (e) {
    console.error('[EnvConfig] 检查配置文件失败:', e);
    hasEnvFile.value = false;
  }
}

// 从后端加载版本映射
async function loadVersionMappings() {
  console.log('[EnvConfig] 开始加载版本映射...');
  try {
    const mappings = await invoke<any>('get_version_mappings');
    console.log('[EnvConfig] 版本映射:', mappings);
    
    // 提取版本信息列表（包含 version、tag、full_name 等完整信息）
    if (mappings.php) {
      phpVersions.value = mappings.php;
      console.log('[EnvConfig] PHP 版本:', phpVersions.value);
    }
    if (mappings.mysql) {
      mysqlVersions.value = mappings.mysql;
      console.log('[EnvConfig] MySQL 版本:', mysqlVersions.value);
    }
    if (mappings.redis) {
      redisVersions.value = mappings.redis;
      console.log('[EnvConfig] Redis 版本:', redisVersions.value);
    }
    if (mappings.nginx) {
      nginxVersions.value = mappings.nginx;
      console.log('[EnvConfig] Nginx 版本:', nginxVersions.value);
    }
  } catch (e) {
    console.error('[EnvConfig] 加载版本映射失败:', e);
    // 使用默认值作为后备
    phpVersions.value = [
      { version: '5.6', tag: '5.6-fpm', full_name: 'php:5.6-fpm', eol: true },
      { version: '7.4', tag: '7.4-fpm', full_name: 'php:7.4-fpm', eol: true },
      { version: '8.0', tag: '8.0-fpm', full_name: 'php:8.0-fpm', eol: true },
      { version: '8.1', tag: '8.1-fpm', full_name: 'php:8.1-fpm', eol: false },
      { version: '8.2', tag: '8.2-fpm', full_name: 'php:8.2-fpm', eol: false },
      { version: '8.3', tag: '8.3-fpm', full_name: 'php:8.3-fpm', eol: false },
      { version: '8.4', tag: '8.4-fpm', full_name: 'php:8.4-fpm', eol: false },
    ];
    mysqlVersions.value = [
      { version: '5.7', tag: '5.7', full_name: 'mysql:5.7', eol: true },
      { version: '8.0', tag: '8.0', full_name: 'mysql:8.0', eol: false },
      { version: '8.4', tag: '8.4', full_name: 'mysql:8.4', eol: false },
    ];
    redisVersions.value = [
      { version: '6.2', tag: '6.2-alpine', full_name: 'redis:6.2-alpine', eol: true },
      { version: '7.0', tag: '7.0-alpine', full_name: 'redis:7.0-alpine', eol: false },
      { version: '7.2', tag: '7.2-alpine', full_name: 'redis:7.2-alpine', eol: false },
      { version: '8.2', tag: '8.2-alpine', full_name: 'redis:8.2-alpine', eol: false },
    ];
    nginxVersions.value = [
      { version: '1.24', tag: '1.24-alpine', full_name: 'nginx:1.24-alpine', eol: true },
      { version: '1.25', tag: '1.25-alpine', full_name: 'nginx:1.25-alpine', eol: false },
      { version: '1.27', tag: '1.27-alpine', full_name: 'nginx:1.27-alpine', eol: false },
    ];
  }
}

// 辅助函数：确保版本在列表中，如果不存在则创建
// version 参数是镜像 tag（如 "8.2-fpm", "8.0", "8.2-alpine"）
function ensureVersionInList(versions: VersionInfo[], version: string, serviceType: string): void {
  // 直接匹配 version 字段
  if (!versions.find(v => v.version === version)) {
    // 如果不存在，创建一个新版本项
    const newVersion: VersionInfo = {
      version: version,
      tag: version,
      full_name: `${serviceType.toLowerCase()}:${version}`,
      eol: false, // 默认标记为未停止维护
    };
    versions.push(newVersion);
    console.log(`[EnvConfig] 动态添加版本: ${version} (${serviceType})`);
  }
}

// 辅助函数：获取 Redis 镜像标签
function getRedisImageTag(version: string): string {
  const info = redisVersions.value.find(v => v.version === version);
  return info ? info.full_name : `redis:${version}`;
}

// 辅助函数：获取 Nginx 镜像标签
function getNginxImageTag(version: string): string {
  const info = nginxVersions.value.find(v => v.version === version);
  return info ? info.full_name : `nginx:${version}`;
}

// 辅助函数：获取 PHP 镜像标签
function getPhpImageTag(version: string): string {
  const info = phpVersions.value.find(v => v.version === version);
  return info ? info.full_name : `php:${version}-fpm`;
}

// 辅助函数：获取 MySQL 镜像标签
function getMysqlImageTag(version: string): string {
  const info = mysqlVersions.value.find(v => v.version === version);
  return info ? info.full_name : `mysql:${version}`;
}

// 错误信息格式化
function formatErrorMessage(error: any): string {
  const errorMsg = String(error);
  
  // Docker 相关错误
  if (errorMsg.includes('Docker') || errorMsg.includes('docker')) {
    if (errorMsg.includes('not running') || errorMsg.includes('unavailable')) {
      return '❌ Docker 未运行\n\n请启动 Docker Desktop 后重试。';
    }
    if (errorMsg.includes('permission')) {
      return '❌ 权限不足\n\n请以管理员身份运行或检查 Docker 权限设置。';
    }
  }
  
  // 端口冲突
  if (errorMsg.includes('端口') || errorMsg.includes('port')) {
    return `⚠️  端口冲突\n\n${errorMsg}\n\n请修改冲突服务的端口号。`;
  }
  
  // 文件操作
  if (errorMsg.includes('读取') || errorMsg.includes('read')) {
    return '❌ 文件读取失败\n\n请检查文件是否存在且有读取权限。';
  }
  if (errorMsg.includes('写入') || errorMsg.includes('write')) {
    return '❌ 文件写入失败\n\n请检查目录是否有写入权限。';
  }
  
  // 配置解析
  if (errorMsg.includes('解析') || errorMsg.includes('parse')) {
    return '❌ 配置文件格式错误\n\n请检查 .env 文件格式是否正确。';
  }
  
  // 默认错误
  return `❌ 操作失败\n\n${errorMsg}`;
}

// 显示错误消息
function showError(message: string) {
  showToast(message, 'error', 5000); // 错误消息显示时间更长
}

async function loadExistingConfig() {
  console.log('[EnvConfig] 开始加载现有配置...');
  try {
    const config = await invoke<EnvConfig | null>('load_existing_config');
    console.log('[EnvConfig] 加载结果:', config);
    
    if (config) {
      // Parse services
      const phpSvcs: ServiceEntry[] = [];
      const mysqlSvcs: ServiceEntry[] = [];
      const redisSvcs: ServiceEntry[] = [];
      const nginxSvcs: ServiceEntry[] = [];
      
      config.services.forEach(s => {
        console.log('[EnvConfig] 解析服务:', s);
        if (s.service_type === 'PHP') {
          phpSvcs.push({ ...s, extensions: s.extensions ? [...s.extensions] : [] });
          // 确保 PHP 版本在列表中
          ensureVersionInList(phpVersions.value, s.version, 'PHP');
        } else if (s.service_type === 'MySQL') {
          mysqlSvcs.push({ ...s });
          // 确保 MySQL 版本在列表中
          ensureVersionInList(mysqlVersions.value, s.version, 'MySQL');
        } else if (s.service_type === 'Redis') {
          redisSvcs.push({ ...s });
          // 确保 Redis 版本在列表中
          ensureVersionInList(redisVersions.value, s.version, 'Redis');
        } else if (s.service_type === 'Nginx') {
          nginxSvcs.push({ ...s });
          // 确保 Nginx 版本在列表中
          ensureVersionInList(nginxVersions.value, s.version, 'Nginx');
        }
      });
      
      console.log('[EnvConfig] PHP 服务:', phpSvcs);
      console.log('[EnvConfig] MySQL 服务:', mysqlSvcs);
      console.log('[EnvConfig] Redis 服务:', redisSvcs);
      console.log('[EnvConfig] Nginx 服务:', nginxSvcs);
      
      phpServices.value = phpSvcs.length > 0 ? phpSvcs : [{
        service_type: 'PHP',
        version: '8.2',
        host_port: 9000,
        extensions: ['pdo_mysql', 'mysqli', 'mbstring', 'gd', 'curl', 'opcache'],
      }];
      
      mysqlServices.value = mysqlSvcs.length > 0 ? mysqlSvcs : [{
        service_type: 'MySQL',
        version: '8.0',
        host_port: 3306,
      }];
      
      redisServices.value = redisSvcs.length > 0 ? redisSvcs : [];
      nginxServices.value = nginxSvcs.length > 0 ? nginxSvcs : [];
      
      sourceDir.value = config.source_dir;
      timezone.value = config.timezone;
      
      // 加载MySQL root密码（如果有）
      if (config.mysql_root_password) {
        mysqlRootPassword.value = config.mysql_root_password;
      }
      
      console.log('[EnvConfig] 配置加载成功');
    } else {
      console.log('[EnvConfig] 未找到现有配置，使用默认值');
      // Default config
      phpServices.value = [{
        service_type: 'PHP',
        version: '8.2',
        host_port: 9000,
        extensions: ['pdo_mysql', 'mysqli', 'mbstring', 'gd', 'curl', 'opcache'],
      }];
      mysqlServices.value = [{
        service_type: 'MySQL',
        version: '8.0',
        host_port: 3306,
      }];
    }
  } catch (e) {
    console.error('[EnvConfig] 加载配置失败:', e);
    // Use defaults
    phpServices.value = [{
      service_type: 'PHP',
      version: '8.2',
      host_port: 9000,
      extensions: ['pdo_mysql', 'mysqli', 'mbstring', 'gd', 'curl', 'opcache'],
    }];
    mysqlServices.value = [{
      service_type: 'MySQL',
      version: '8.0',
      host_port: 3306,
    }];
  }
}

// Port conflict detection (仅检测 MySQL、Redis、Nginx 的宿主机端口)
const allPorts = computed(() => {
  const ports: { service: string; port: number }[] = [];
  // PHP 服务不需要宿主机端口映射，跳过
  mysqlServices.value.forEach((s, i) => {
    ports.push({ service: `MySQL ${s.version} (#${i + 1})`, port: s.host_port });
  });
  redisServices.value.forEach((s, i) => {
    ports.push({ service: `Redis ${s.version} (#${i + 1})`, port: s.host_port });
  });
  nginxServices.value.forEach((s, i) => {
    ports.push({ service: `Nginx ${s.version} (#${i + 1})`, port: s.host_port });
  });
  return ports;
});

const portConflicts = computed(() => {
  const seen = new Map<number, string>();
  const conflicts: string[] = [];
  for (const { service, port } of allPorts.value) {
    if (seen.has(port)) {
      conflicts.push(`端口 ${port} 冲突：${seen.get(port)} 和 ${service}`);
    } else {
      seen.set(port, service);
    }
  }
  return conflicts;
});

// Build config
function buildConfig(): EnvConfig {
  const services: ServiceEntry[] = [];
  phpServices.value.forEach(s => {
    services.push({ ...s, extensions: [...(s.extensions || [])] });
  });
  mysqlServices.value.forEach(s => {
    services.push({ ...s });
  });
  redisServices.value.forEach(s => {
    services.push({ ...s });
  });
  nginxServices.value.forEach(s => {
    services.push({ ...s });
  });
  return { 
    services, 
    source_dir: sourceDir.value, 
    timezone: timezone.value,
    mysql_root_password: mysqlRootPassword.value === 'root' ? undefined : mysqlRootPassword.value
  };
}

// Add PHP version
function addPhpVersion() {
  const usedVersions = phpServices.value.map(s => s.version);
  const available = phpVersions.value.filter(v => !usedVersions.includes(v.version));
  if (available.length === 0) return;
  phpServices.value.push({
    service_type: 'PHP',
    version: available[0].version,
    host_port: 9000 + phpServices.value.length,
    extensions: ['pdo_mysql', 'mysqli', 'mbstring', 'curl'],
  });
}

function removePhpVersion(index: number) {
  if (phpServices.value.length <= 1) return;
  phpServices.value.splice(index, 1);
}

// Add MySQL version
function addMysqlVersion() {
  const usedVersions = mysqlServices.value.map(s => s.version);
  const available = mysqlVersions.value.filter(v => !usedVersions.includes(v.version));
  if (available.length === 0) return;
  mysqlServices.value.push({
    service_type: 'MySQL',
    version: available[0].version,
    host_port: 3306 + mysqlServices.value.length,
  });
}

function removeMysqlVersion(index: number) {
  if (mysqlServices.value.length <= 1) return;
  mysqlServices.value.splice(index, 1);
}

// Add Redis version
function addRedisVersion() {
  const usedVersions = redisServices.value.map(s => s.version);
  const available = redisVersions.value.filter(v => !usedVersions.includes(v.version));
  if (available.length === 0) return;
  redisServices.value.push({
    service_type: 'Redis',
    version: available[0].version,
    host_port: 6379 + redisServices.value.length,
  });
}

function removeRedisVersion(index: number) {
  redisServices.value.splice(index, 1);
}

// Add Nginx version
function addNginxVersion() {
  const usedVersions = nginxServices.value.map(s => s.version);
  const available = nginxVersions.value.filter(v => !usedVersions.includes(v.version));
  if (available.length === 0) return;
  nginxServices.value.push({
    service_type: 'Nginx',
    version: available[0].version,
    host_port: 80 + nginxServices.value.length,
  });
}

function removeNginxVersion(index: number) {
  nginxServices.value.splice(index, 1);
}

function toggleExtension(phpIndex: number, ext: string) {
  const service = phpServices.value[phpIndex];
  if (!service.extensions) service.extensions = [];
  const idx = service.extensions.indexOf(ext);
  if (idx >= 0) {
    service.extensions.splice(idx, 1);
  } else {
    service.extensions.push(ext);
  }
}

// Preview
async function handlePreview() {
  if (portConflicts.value.length > 0) {
    showError(portConflicts.value.join('\n'));
    return;
  }
  loading.value = true;
  try {
    const config = buildConfig();
    const [envContent, composeContent] = await Promise.all([
      invoke<string>('generate_env_config', { config }),
      invoke<string>('preview_compose', { config }),
    ]);
    previewEnv.value = envContent;
    previewCompose.value = composeContent;
    showPreviewModal.value = true;
  } catch (e) {
    showError(formatErrorMessage(e));
  } finally {
    loading.value = false;
  }
}

// Apply
async function handleApply() {
  if (portConflicts.value.length > 0) {
    showError(portConflicts.value.join('\n'));
    return;
  }
  
  // 检查配置文件是否存在
  let enableBackup = false;
  try {
    const existingFiles = await invoke<string[]>('check_config_files_exist');
    if (existingFiles.length > 0) {
      // 有文件存在，显示确认对话框
      const fileList = existingFiles.map(f => `• ${f}`).join('\n');
      const result = await showConfirm({
        title: '配置文件已存在',
        message: `检测到以下配置文件已存在：\n\n${fileList}\n\n继续操作将覆盖这些文件，是否继续？`,
        confirmText: '应用配置',
        cancelText: '取消',
        type: 'warning',
        checkboxLabel: '备份现有配置（自动回滚机制，失败会恢复原状）',
        checkboxDefault: true
      });
      
      // 如果返回的是对象（有复选框），解构获取结果
      const confirmed = typeof result === 'object' ? result.confirmed : result;
      if (!confirmed) {
        return; // 用户取消
      }
      
      // 获取复选框的值
      if (typeof result === 'object') {
        enableBackup = result.checkboxValue;
      }
    }
  } catch (e) {
    console.error('检查配置文件失败:', e);
    // 如果检查失败，继续执行（不阻断用户操作）
  }
  
  applying.value = true;
  showNginxHint.value = false;
  try {
    const config = buildConfig();
    const backedUpFiles = await invoke<string[]>('apply_env_config', { config, enableBackup });
    
    // 显示成功消息
    let successMsg = import.meta.env.DEV 
      ? '配置已成功应用！配置文件已生成在项目根目录。' 
      : '配置已成功应用！配置文件已生成在程序所在目录。';
    
    if (backedUpFiles && backedUpFiles.length > 0) {
      successMsg += `\n\n✅ 已备份 ${backedUpFiles.length} 个文件/目录：\n` + 
        backedUpFiles.map(f => `• ${f}`).join('\n');
    } else if (enableBackup) {
      successMsg += '\n\n⚠️  注意：部分文件备份失败（可能文件被占用），请关闭相关程序后重试。';
    }
    
    showToast(successMsg, 'success', 6000);
    showPreviewModal.value = false;
    
    // 更新 .env 文件存在状态
    hasEnvFile.value = true;
    
    // 检查是否同时启用了 PHP 和 Nginx
    const hasPHP = phpServices.value.length > 0;
    const hasNginx = nginxServices.value.length > 0;
    
    if (hasPHP && hasNginx) {
      // 获取所有 PHP 服务的容器地址（容器名:端口）
      phpContainerNames.value = phpServices.value.map(service => {
        const ver = service.version.replace(/\./g, '');
        return `ps-php${ver}:9000`;
      });
      
      // 获取所有 Nginx 服务的信息
      nginxServicesList.value = nginxServices.value.map(service => {
        const ver = service.version.replace(/\./g, '');
        return {
          name: `nginx${ver}`,
          version: service.version,
          port: service.port
        };
      });
      
      showNginxHint.value = true;
    }
  } catch (e) {
    showError(formatErrorMessage(e));
  } finally {
    applying.value = false;
  }
}

// 打开 Nginx 配置目录
async function openNginxConfigDir(serviceName?: string) {
  try {
    // 如果没有指定服务名，默认打开第一个 Nginx 的配置目录
    const targetService = serviceName || (nginxServicesList.value.length > 0 ? nginxServicesList.value[0].name : 'nginx127');
    await invoke('open_service_config', { serviceName: targetService });
    showToast(`已打开 ${targetService} 配置目录`, 'success');
  } catch (e) {
    console.error('打开目录失败:', e);
    const targetService = serviceName || (nginxServicesList.value.length > 0 ? nginxServicesList.value[0].name : 'nginx127');
    showToast(`打开目录失败，请手动打开 services/${targetService}/conf.d/`, 'error');
  }
}

// Start environment
async function handleStart() {
  showStartConfirm.value = true;
}

async function confirmStart() {
  showStartConfirm.value = false;
  starting.value = true;
  try {
    const result = await invoke<string>('start_environment');
    showToast('环境启动成功！\n' + result, 'success', 5000);
  } catch (e) {
    showError(formatErrorMessage(e));
  } finally {
    starting.value = false;
  }
}

const goToMirrorSettings = () => {
  showStartConfirm.value = false;
  emit('request-switch-tab', 'mirrors-unified');
};
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden">
    <header class="flex flex-col lg:flex-row justify-between items-start lg:items-center gap-4 mb-6">
      <div>
        <h1 class="text-2xl sm:text-3xl font-bold">环境配置</h1>
        <p class="text-slate-400 text-xs sm:text-sm mt-1">可视化配置 .env 和 docker-compose.yml</p>
      </div>
      <div class="flex flex-col sm:flex-row gap-3 w-full lg:w-auto">
        <button
          @click="handlePreview"
          :disabled="loading"
          class="w-full sm:w-auto px-5 py-2 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg font-medium transition disabled:opacity-50"
        >
          {{ loading ? '生成中...' : '预览配置' }}
        </button>
        <button
          @click="handleApply"
          :disabled="applying || portConflicts.length > 0"
          class="w-full sm:w-auto px-5 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg font-medium transition disabled:opacity-50"
        >
          {{ applying ? '应用中...' : '应用配置' }}
        </button>
        <button
          @click="handleStart"
          :disabled="starting || !hasEnvFile"
          class="w-full sm:w-auto px-5 py-2 bg-green-600 hover:bg-green-700 rounded-lg font-medium transition disabled:opacity-50 disabled:cursor-not-allowed"
          :title="!hasEnvFile ? '请先应用配置生成 .env 文件' : ''"
        >
          {{ starting ? '启动中...' : '一键启动' }}
        </button>
      </div>
    </header>
    
    <!-- Nginx 配置提示 -->
    <div v-if="showNginxHint" class="mb-4 p-4 sm:p-5 bg-blue-500/10 border border-blue-500/20 rounded-xl">
      <div class="flex flex-col sm:flex-row items-start gap-3">
        <div class="flex-shrink-0">
          <svg xmlns="http://www.w3.org/2000/svg" class="w-6 h-6 text-blue-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        </div>
        <div class="flex-1">
          <h3 class="text-base font-semibold text-blue-300 mb-2">Nginx 配置提醒</h3>
          <p class="text-sm text-slate-300 mb-3">
            检测到您同时启用了 PHP 和 Nginx 服务。Nginx 需要配置正确的 PHP-FPM 上游地址。
          </p>
          
          <div class="bg-slate-900 rounded-lg p-3 mb-3 border border-slate-700">
            <p class="text-xs text-slate-400 mb-2">📌 PHP 容器地址列表（用于 Nginx 配置）：</p>
            <div class="space-y-1">
              <div v-for="(name, index) in phpContainerNames" :key="index" class="flex items-center gap-2">
                <span class="text-xs text-slate-500 font-mono">{{ index + 1 }}.</span>
                <code class="text-sm text-emerald-400 font-mono">{{ name }}</code>
              </div>
            </div>
          </div>
          
          <!-- 多 Nginx 版本提示 -->
          <div v-if="nginxServicesList.length > 1" class="bg-amber-900/20 rounded-lg p-3 mb-3 border border-amber-500/20">
            <p class="text-xs text-amber-300 mb-2">⚠️ 检测到多个 Nginx 版本：</p>
            <div class="space-y-2">
              <div v-for="(nginx, index) in nginxServicesList" :key="index" class="flex flex-col sm:flex-row sm:items-center gap-2 text-sm">
                <div class="flex items-center gap-2">
                  <span class="text-xs text-slate-500 font-mono">{{ index + 1 }}.</span>
                  <code class="text-sm text-blue-400 font-mono">{{ nginx.name }}</code>
                  <span class="text-xs text-slate-500">(v{{ nginx.version }})</span>
                  <span v-if="nginx.port" class="text-xs text-slate-500">- 端口 {{ nginx.port }}</span>
                </div>
                <button
                  @click="openNginxConfigDir(nginx.name)"
                  class="sm:ml-auto px-3 py-1 bg-blue-600/20 hover:bg-blue-600/30 text-blue-400 rounded text-xs transition border border-blue-600/30 whitespace-nowrap"
                >
                  打开配置目录
                </button>
              </div>
            </div>
          </div>
          
          <div class="space-y-2 text-sm text-slate-300">
            <p><strong class="text-blue-300">配置步骤：</strong></p>
            <ol class="list-decimal list-inside space-y-1 ml-2 text-slate-400">
              <li v-if="nginxServicesList.length === 1">
                编辑文件：<code class="text-xs bg-slate-800 px-1 rounded">services/{{ nginxServicesList[0].name }}/conf.d/default.conf</code>
              </li>
              <li v-else>
                为每个 Nginx 版本编辑对应的配置文件（见上方列表）
              </li>
              <li>找到 <code class="text-xs bg-slate-800 px-1 rounded">fastcgi_pass</code> 行（默认值为 <code class="text-xs bg-slate-800 px-1 rounded">php:9000</code>）</li>
              <li>修改为：<code class="text-xs bg-slate-800 px-1 rounded text-emerald-400">fastcgi_pass [容器地址];</code>（选择上面的某个容器地址，如 <code class="text-emerald-400">ps-php85:9000</code>）</li>
              <li class="text-xs text-slate-500 mt-1">💡 提示：如果使用了多个 PHP 版本，可以为不同的 server 块配置不同的 fastcgi_pass 地址</li>
            </ol>
          </div>
          
          <div class="mt-4 flex flex-col sm:flex-row gap-2">
            <button
              v-if="nginxServicesList.length === 1"
              @click="openNginxConfigDir()"
              class="w-full sm:w-auto px-4 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-sm font-medium transition flex items-center justify-center gap-2"
            >
              <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
              </svg>
              打开配置目录
            </button>
            <button
              @click="showNginxHint = false"
              class="w-full sm:w-auto px-4 py-2 bg-slate-700 hover:bg-slate-600 rounded-lg text-sm font-medium transition"
            >
              我知道了
            </button>
          </div>
        </div>
      </div>
    </div>
    
    <div v-if="portConflicts.length > 0" class="mb-4 p-4 bg-amber-500/10 border border-amber-500/20 rounded-xl text-amber-400 text-sm">
      <div class="font-bold mb-1">端口冲突</div>
      <div v-for="c in portConflicts" :key="c">{{ c }}</div>
    </div>

    <div class="flex-1 overflow-y-auto pr-1 sm:pr-2 space-y-4 sm:space-y-6">
      <!-- PHP Services -->
      <section class="bg-slate-900 border border-slate-800 rounded-xl p-4 sm:p-6">
        <div class="flex justify-between items-center mb-4">
          <h2 class="text-lg font-bold">🐘 PHP 服务</h2>
          <button @click="addPhpVersion" class="text-sm px-3 py-1 bg-blue-600/20 text-blue-400 border border-blue-600/30 rounded-lg hover:bg-blue-600 hover:text-white transition">
            + 添加版本
          </button>
        </div>
        <div v-for="(php, idx) in phpServices" :key="idx" class="mb-4 sm:mb-6 p-3 sm:p-4 bg-slate-800/50 border border-slate-700 rounded-lg">
          <div class="flex flex-col sm:flex-row items-start sm:items-center gap-3 sm:gap-4 mb-3">
            <div class="flex-1 w-full sm:w-auto">
              <label class="block text-xs text-slate-400 mb-1">
                PHP 版本
                <span class="text-slate-500 ml-1 hidden sm:inline">(将使用镜像: {{ getPhpImageTag(php.version) }})</span>
              </label>
              <select v-model="php.version" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500">
                <option v-for="v in phpVersions" :key="v.version" :value="v.version">
                  PHP {{ v.version }} → {{ v.tag }}
                  <span v-if="v.eol" class="text-amber-400">(EOL)</span>
                </option>
              </select>
            </div>
            <button v-if="phpServices.length > 1" @click="removePhpVersion(idx)" class="w-full sm:w-auto mt-2 sm:mt-5 text-rose-400 hover:text-rose-300 text-sm">删除</button>
          </div>
          <div>
            <label class="block text-xs text-slate-400 mb-2">PHP 扩展</label>
            <div class="flex flex-wrap gap-2">
              <label
                v-for="ext in commonExtensions"
                :key="ext"
                class="flex items-center gap-1.5 text-xs px-2 py-1 rounded cursor-pointer transition"
                :class="php.extensions?.includes(ext) ? 'bg-blue-600/20 text-blue-400 border border-blue-500/30' : 'bg-slate-800 text-slate-500 border border-slate-700 hover:border-slate-600'"
              >
                <input type="checkbox" :checked="php.extensions?.includes(ext)" @change="toggleExtension(idx, ext)" class="hidden" />
                {{ ext }}
              </label>
            </div>
          </div>
        </div>
      </section>

      <!-- MySQL -->
      <section class="bg-slate-900 border border-slate-800 rounded-xl p-4 sm:p-6">
        <div class="flex justify-between items-center mb-4">
          <h2 class="text-lg font-bold">🐬 MySQL 服务</h2>
          <button @click="addMysqlVersion" class="text-sm px-3 py-1 bg-blue-600/20 text-blue-400 border border-blue-600/30 rounded-lg hover:bg-blue-600 hover:text-white transition">
            + 添加版本
          </button>
        </div>
        <div v-for="(mysql, idx) in mysqlServices" :key="idx" class="mb-3 sm:mb-4 p-3 sm:p-4 bg-slate-800/50 border border-slate-700 rounded-lg">
          <div class="flex flex-col sm:flex-row items-start sm:items-center gap-3 sm:gap-4">
            <div class="flex-1 w-full sm:w-auto">
              <label class="block text-xs text-slate-400 mb-1">
                MySQL 版本
                <span class="text-slate-500 ml-1 hidden sm:inline">(将使用镜像: {{ getMysqlImageTag(mysql.version) }})</span>
              </label>
              <select v-model="mysql.version" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500">
                <option v-for="v in mysqlVersions" :key="v.version" :value="v.version">
                  MySQL {{ v.version }} → {{ v.tag }}
                  <span v-if="v.eol" class="text-amber-400">(EOL)</span>
                </option>
              </select>
            </div>
            <div class="w-full sm:w-32">
              <label class="block text-xs text-slate-400 mb-1">宿主机端口</label>
              <input v-model.number="mysql.host_port" type="number" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500" />
            </div>
            <button v-if="mysqlServices.length > 1" @click="removeMysqlVersion(idx)" class="w-full sm:w-auto mt-2 sm:mt-5 text-rose-400 hover:text-rose-300 text-sm">删除</button>
          </div>
        </div>
        
        <!-- MySQL Root 密码配置 -->
        <div class="mt-4 p-3 sm:p-4 bg-slate-800/30 border border-slate-700/50 rounded-lg">
          <div class="flex flex-col sm:flex-row items-start sm:items-center gap-3 sm:gap-4">
            <div class="flex-1 w-full sm:w-64">
              <label class="block text-xs text-slate-400 mb-1">MySQL Root 密码</label>
              <input 
                v-model="mysqlRootPassword" 
                type="password" 
                placeholder="root"
                class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500" 
              />
            </div>
            <div class="flex-1">
              <p class="text-xs text-slate-500">留空或输入 "root" 将使用默认密码</p>
            </div>
          </div>
        </div>
      </section>

      <!-- Redis -->
      <section class="bg-slate-900 border border-slate-800 rounded-xl p-4 sm:p-6">
        <div class="flex justify-between items-center mb-4">
          <h2 class="text-lg font-bold">🔴 Redis 服务</h2>
          <button @click="addRedisVersion" class="text-sm px-3 py-1 bg-blue-600/20 text-blue-400 border border-blue-600/30 rounded-lg hover:bg-blue-600 hover:text-white transition">
            + 添加版本
          </button>
        </div>
        <div v-for="(redis, idx) in redisServices" :key="idx" class="mb-3 sm:mb-4 p-3 sm:p-4 bg-slate-800/50 border border-slate-700 rounded-lg">
          <div class="flex flex-col sm:flex-row items-start sm:items-center gap-3 sm:gap-4">
            <div class="flex-1 w-full sm:w-auto">
              <label class="block text-xs text-slate-400 mb-1">
                Redis 版本
                <span class="text-slate-500 ml-1 hidden sm:inline">(将使用镜像: {{ getRedisImageTag(redis.version) }})</span>
              </label>
              <select v-model="redis.version" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500">
                <option v-for="v in redisVersions" :key="v.version" :value="v.version">
                  Redis {{ v.version }} → {{ v.tag }}
                  <span v-if="v.eol" class="text-amber-400">(EOL)</span>
                </option>
              </select>
            </div>
            <div class="w-full sm:w-32">
              <label class="block text-xs text-slate-400 mb-1">宿主机端口</label>
              <input v-model.number="redis.host_port" type="number" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500" />
            </div>
            <button @click="removeRedisVersion(idx)" class="w-full sm:w-auto mt-2 sm:mt-5 text-rose-400 hover:text-rose-300 text-sm">删除</button>
          </div>
        </div>
        <div v-if="redisServices.length === 0" class="text-center py-8 text-slate-500 text-sm">
          点击上方“+ 添加版本”按钮添加 Redis 服务
        </div>
      </section>

      <!-- Nginx -->
      <section class="bg-slate-900 border border-slate-800 rounded-xl p-4 sm:p-6">
        <div class="flex justify-between items-center mb-4">
          <h2 class="text-lg font-bold">🚀 Nginx 服务</h2>
          <button @click="addNginxVersion" class="text-sm px-3 py-1 bg-blue-600/20 text-blue-400 border border-blue-600/30 rounded-lg hover:bg-blue-600 hover:text-white transition">
            + 添加版本
          </button>
        </div>
        <div v-for="(nginx, idx) in nginxServices" :key="idx" class="mb-3 sm:mb-4 p-3 sm:p-4 bg-slate-800/50 border border-slate-700 rounded-lg">
          <div class="flex flex-col sm:flex-row items-start sm:items-center gap-3 sm:gap-4">
            <div class="flex-1 w-full sm:w-auto">
              <label class="block text-xs text-slate-400 mb-1">
                Nginx 版本
                <span class="text-slate-500 ml-1 hidden sm:inline">(将使用镜像: {{ getNginxImageTag(nginx.version) }})</span>
              </label>
              <select v-model="nginx.version" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500">
                <option v-for="v in nginxVersions" :key="v.version" :value="v.version">
                  Nginx {{ v.version }} → {{ v.tag }}
                  <span v-if="v.eol" class="text-amber-400">(EOL)</span>
                </option>
              </select>
            </div>
            <div class="w-full sm:w-32">
              <label class="block text-xs text-slate-400 mb-1">宿主机端口</label>
              <input v-model.number="nginx.host_port" type="number" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500" />
            </div>
            <button @click="removeNginxVersion(idx)" class="w-full sm:w-auto mt-2 sm:mt-5 text-rose-400 hover:text-rose-300 text-sm">删除</button>
          </div>
        </div>
        <div v-if="nginxServices.length === 0" class="text-center py-8 text-slate-500 text-sm">
          点击上方“+ 添加版本”按钮添加 Nginx 服务
        </div>
      </section>

      <!-- General Settings -->
      <section class="bg-slate-900 border border-slate-800 rounded-xl p-4 sm:p-6">
        <h2 class="text-lg font-bold mb-4">🔧 通用设置</h2>
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          <div>
            <label class="block text-xs text-slate-400 mb-1">工作目录</label>
            <input 
              :value="workspacePath" 
              readonly
              class="w-full bg-slate-800/50 border border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-300 cursor-not-allowed"
            />
          </div>
          <div>
            <label class="block text-xs text-slate-400 mb-1">项目源码目录</label>
            <input v-model="sourceDir" type="text" placeholder="./www" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500" />
          </div>
          <div>
            <label class="block text-xs text-slate-400 mb-1">时区</label>
            <select v-model="timezone" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500">
              <option value="Asia/Shanghai">Asia/Shanghai</option>
              <option value="Asia/Tokyo">Asia/Tokyo</option>
              <option value="Asia/Hong_Kong">Asia/Hong_Kong</option>
              <option value="UTC">UTC</option>
              <option value="America/New_York">America/New_York</option>
              <option value="Europe/London">Europe/London</option>
            </select>
          </div>
        </div>
      </section>
    </div>

    <!-- Preview Modal -->
    <div v-if="showPreviewModal" class="fixed inset-0 bg-black/70 flex items-center justify-center z-50 p-3 sm:p-4" @click.self="showPreviewModal = false">
      <div class="bg-slate-900 border border-slate-700 rounded-xl w-full max-w-4xl sm:max-w-6xl mx-auto max-h-[90vh] flex flex-col">
        <div class="flex justify-between items-center p-4 sm:p-6 border-b border-slate-700">
          <h2 class="text-lg sm:text-xl font-bold">📄 配置预览</h2>
          <button @click="showPreviewModal = false" class="text-slate-400 hover:text-white text-2xl">&times;</button>
        </div>
        <div class="flex-1 overflow-y-auto p-4 sm:p-6">
          <div class="grid grid-cols-1 lg:grid-cols-2 gap-3 sm:gap-4">
            <div>
              <div class="text-xs text-slate-400 mb-2 uppercase tracking-wider">.env</div>
              <pre class="bg-black/40 p-3 sm:p-4 rounded-lg text-xs text-green-300/80 border border-slate-700 max-h-80 sm:max-h-96 overflow-y-auto font-mono whitespace-pre-wrap">{{ previewEnv }}</pre>
            </div>
            <div>
              <div class="text-xs text-slate-400 mb-2 uppercase tracking-wider">docker-compose.yml</div>
              <pre class="bg-black/40 p-3 sm:p-4 rounded-lg text-xs text-blue-300/80 border border-slate-700 max-h-80 sm:max-h-96 overflow-y-auto font-mono whitespace-pre-wrap">{{ previewCompose }}</pre>
            </div>
          </div>
        </div>
        <div class="p-4 sm:p-6 border-t border-slate-700 flex flex-col sm:flex-row justify-end gap-3">
          <button @click="showPreviewModal = false" class="w-full sm:w-auto px-5 py-2 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg font-medium transition">
            关闭
          </button>
          <button @click="handleApply" :disabled="applying" class="w-full sm:w-auto px-5 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg font-medium transition disabled:opacity-50">
            {{ applying ? '应用中...' : '应用配置' }}
          </button>
        </div>
      </div>
    </div>

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
