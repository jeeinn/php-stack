<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { ServiceEntry, EnvConfig } from '../types/env-config';

// Available versions (将从后端动态加载)
const phpVersions = ref<string[]>([]);
const mysqlVersions = ref<string[]>([]);
const redisVersions = ref<string[]>([]);
const nginxVersions = ref<string[]>([]);

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

const loading = ref(false);
const applying = ref(false);
const starting = ref(false);
const error = ref<string | null>(null);
const successMsg = ref<string | null>(null);
const previewEnv = ref('');
const previewCompose = ref('');
const showPreviewModal = ref(false);

// 确认对话框状态
const showConfirmDialog = ref(false);
const confirmMessage = ref('');
const confirmTitle = ref('');
const confirmResolve = ref<((value: boolean) => void) | null>(null);

// Nginx 配置提示状态
const showNginxHint = ref(false);
const phpContainerName = ref('');

// Load existing config on mount
onMounted(async () => {
  await loadVersionMappings();
  await loadExistingConfig();
});

// 从后端加载版本映射
async function loadVersionMappings() {
  console.log('[EnvConfig] 开始加载版本映射...');
  try {
    const mappings = await invoke<any>('get_version_mappings');
    console.log('[EnvConfig] 版本映射:', mappings);
    
    // 提取版本号列表
    if (mappings.php) {
      phpVersions.value = mappings.php.map((v: any) => v.version);
      console.log('[EnvConfig] PHP 版本:', phpVersions.value);
    }
    if (mappings.mysql) {
      mysqlVersions.value = mappings.mysql.map((v: any) => v.version);
      console.log('[EnvConfig] MySQL 版本:', mysqlVersions.value);
    }
    if (mappings.redis) {
      redisVersions.value = mappings.redis.map((v: any) => v.tag); // Redis 使用 tag
      console.log('[EnvConfig] Redis 版本:', redisVersions.value);
    }
    if (mappings.nginx) {
      nginxVersions.value = mappings.nginx.map((v: any) => v.tag); // Nginx 使用 tag
      console.log('[EnvConfig] Nginx 版本:', nginxVersions.value);
    }
  } catch (e) {
    console.error('[EnvConfig] 加载版本映射失败:', e);
    // 使用默认值作为后备
    phpVersions.value = ['5.6', '7.4', '8.0', '8.1', '8.2', '8.3', '8.4', '8.5'];
    mysqlVersions.value = ['5.7', '8.0', '8.4'];
    redisVersions.value = ['6.2-alpine', '7.0-alpine', '7.2-alpine'];
    nginxVersions.value = ['1.24-alpine', '1.25-alpine', '1.26-alpine', '1.27-alpine'];
  }
}

// 辅助函数：确保版本在列表中，如果不存在则添加
function ensureVersionInList(versions: string[], version: string): void {
  if (!versions.includes(version)) {
    versions.push(version);
    console.log(`[EnvConfig] 动态添加版本到列表: ${version}`);
  }
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
          ensureVersionInList(phpVersions.value, s.version);
        } else if (s.service_type === 'MySQL') {
          mysqlSvcs.push({ ...s });
          // 确保 MySQL 版本在列表中
          ensureVersionInList(mysqlVersions.value, s.version);
        } else if (s.service_type === 'Redis') {
          redisSvcs.push({ ...s });
          // 确保 Redis 版本在列表中
          ensureVersionInList(redisVersions.value, s.version);
        } else if (s.service_type === 'Nginx') {
          nginxSvcs.push({ ...s });
          // 确保 Nginx 版本在列表中
          ensureVersionInList(nginxVersions.value, s.version);
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

// Port conflict detection
const allPorts = computed(() => {
  const ports: { service: string; port: number }[] = [];
  phpServices.value.forEach((s, i) => {
    ports.push({ service: `PHP ${s.version} (#${i + 1})`, port: s.host_port });
  });
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
  return { services, source_dir: sourceDir.value, timezone: timezone.value };
}

// Add PHP version
function addPhpVersion() {
  const usedVersions = phpServices.value.map(s => s.version);
  const available = phpVersions.value.filter(v => !usedVersions.includes(v));
  if (available.length === 0) return;
  phpServices.value.push({
    service_type: 'PHP',
    version: available[0],
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
  const available = mysqlVersions.value.filter(v => !usedVersions.includes(v));
  if (available.length === 0) return;
  mysqlServices.value.push({
    service_type: 'MySQL',
    version: available[0],
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
  const available = redisVersions.value.filter(v => !usedVersions.includes(v));
  if (available.length === 0) return;
  redisServices.value.push({
    service_type: 'Redis',
    version: available[0],
    host_port: 6379 + redisServices.value.length,
  });
}

function removeRedisVersion(index: number) {
  redisServices.value.splice(index, 1);
}

// Add Nginx version
function addNginxVersion() {
  const usedVersions = nginxServices.value.map(s => s.version);
  const available = nginxVersions.value.filter(v => !usedVersions.includes(v));
  if (available.length === 0) return;
  nginxServices.value.push({
    service_type: 'Nginx',
    version: available[0],
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
    error.value = portConflicts.value.join('\n');
    return;
  }
  loading.value = true;
  error.value = null;
  successMsg.value = null;
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
    error.value = e as string;
  } finally {
    loading.value = false;
  }
}

// 显示确认对话框（Promise 封装）
function showConfirmDialogFn(title: string, message: string): Promise<boolean> {
  return new Promise((resolve) => {
    confirmTitle.value = title;
    confirmMessage.value = message;
    confirmResolve.value = resolve;
    showConfirmDialog.value = true;
  });
}

// 处理确认对话框按钮
function handleConfirmDialog(result: boolean) {
  showConfirmDialog.value = false;
  if (confirmResolve.value) {
    confirmResolve.value(result);
    confirmResolve.value = null;
  }
}

// Apply
async function handleApply() {
  if (portConflicts.value.length > 0) {
    error.value = portConflicts.value.join('\n');
    return;
  }
  
  // 检查配置文件是否存在
  try {
    const existingFiles = await invoke<string[]>('check_config_files_exist');
    if (existingFiles.length > 0) {
      // 有文件存在，显示确认对话框
      const fileList = existingFiles.map(f => `• ${f}`).join('\n');
      const confirmed = await showConfirmDialogFn(
        '配置文件已存在',
        `检测到以下配置文件已存在：\n\n${fileList}\n\n继续操作将覆盖这些文件，是否继续？`
      );
      if (!confirmed) {
        return; // 用户取消
      }
    }
  } catch (e) {
    console.error('检查配置文件失败:', e);
    // 如果检查失败，继续执行（不阻断用户操作）
  }
  
  applying.value = true;
  error.value = null;
  successMsg.value = null;
  showNginxHint.value = false;
  try {
    const config = buildConfig();
    await invoke('apply_env_config', { config });
    
    // 显示成功消息
    successMsg.value = import.meta.env.DEV 
      ? '配置已成功应用！配置文件已生成在项目根目录。' 
      : '配置已成功应用！配置文件已生成在程序所在目录。';
    showPreviewModal.value = false;
    
    // 检查是否同时启用了 PHP 和 Nginx
    const hasPHP = phpServices.value.length > 0;
    const hasNginx = nginxServices.value.length > 0;
    
    if (hasPHP && hasNginx) {
      // 获取第一个 PHP 服务的容器名称
      const firstPHP = phpServices.value[0];
      const ver = firstPHP.version.replace(/\./g, '');
      phpContainerName.value = `ps-php${ver}`;
      showNginxHint.value = true;
    }
  } catch (e) {
    error.value = e as string;
  } finally {
    applying.value = false;
  }
}

// 复制 Nginx 配置
async function copyNginxConfig() {
  const config = `location ~ \.php$ {
    fastcgi_pass ${phpContainerName.value}:9000;
    fastcgi_index index.php;
    fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;
    include fastcgi_params;
}`;
  
  try {
    await navigator.clipboard.writeText(config);
    successMsg.value = 'Nginx 配置已复制到剪贴板！';
    setTimeout(() => { successMsg.value = null; }, 3000);
  } catch (e) {
    console.error('复制失败:', e);
    error.value = '复制失败，请手动配置';
  }
}

// Start environment
async function handleStart() {
  starting.value = true;
  error.value = null;
  successMsg.value = null;
  try {
    const result = await invoke<string>('start_environment');
    successMsg.value = '环境启动成功！\n' + result;
  } catch (e) {
    error.value = e as string;
  } finally {
    starting.value = false;
  }
}
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden">
    <header class="flex justify-between items-center mb-6">
      <div>
        <h1 class="text-3xl font-bold">环境配置</h1>
        <p class="text-slate-400 text-sm mt-1">可视化配置 .env 和 docker-compose.yml</p>
      </div>
      <div class="flex gap-3">
        <button
          @click="handlePreview"
          :disabled="loading"
          class="px-5 py-2 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg font-medium transition disabled:opacity-50"
        >
          {{ loading ? '生成中...' : '预览配置' }}
        </button>
        <button
          @click="handleApply"
          :disabled="applying || portConflicts.length > 0"
          class="px-5 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg font-medium transition disabled:opacity-50"
        >
          {{ applying ? '应用中...' : '应用配置' }}
        </button>
        <button
          @click="handleStart"
          :disabled="starting"
          class="px-5 py-2 bg-green-600 hover:bg-green-700 rounded-lg font-medium transition disabled:opacity-50"
        >
          {{ starting ? '启动中...' : '一键启动' }}
        </button>
      </div>
    </header>

    <!-- Error / Success Alert -->
    <div v-if="error" class="mb-4 p-4 bg-rose-500/10 border border-rose-500/20 rounded-xl text-rose-400 text-sm">
      <pre class="whitespace-pre-wrap">{{ error }}</pre>
    </div>
    <div v-if="successMsg" class="mb-4 p-4 bg-green-500/10 border border-green-500/20 rounded-xl text-green-400 text-sm">
      <pre class="whitespace-pre-wrap">{{ successMsg }}</pre>
    </div>
    
    <!-- Nginx 配置提示 -->
    <div v-if="showNginxHint" class="mb-4 p-5 bg-blue-500/10 border border-blue-500/20 rounded-xl">
      <div class="flex items-start gap-3">
        <div class="flex-shrink-0">
          <svg xmlns="http://www.w3.org/2000/svg" class="w-6 h-6 text-blue-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        </div>
        <div class="flex-1">
          <h3 class="text-base font-semibold text-blue-300 mb-2">⚙️ Nginx 配置提醒</h3>
          <p class="text-sm text-slate-300 mb-3">
            检测到您同时启用了 PHP 和 Nginx 服务。Nginx 需要配置正确的 PHP-FPM 上游地址。
          </p>
          
          <div class="bg-slate-900 rounded-lg p-3 mb-3 border border-slate-700">
            <p class="text-xs text-slate-400 mb-2">📌 当前 PHP 容器名称：</p>
            <code class="text-sm text-emerald-400 font-mono">{{ phpContainerName }}</code>
          </div>
          
          <div class="space-y-2 text-sm text-slate-300">
            <p><strong class="text-blue-300">配置步骤：</strong></p>
            <ol class="list-decimal list-inside space-y-1 ml-2 text-slate-400">
              <li>编辑文件：<code class="text-xs bg-slate-800 px-1 rounded">services/nginx/conf.d/default.conf</code></li>
              <li>找到 <code class="text-xs bg-slate-800 px-1 rounded">fastcgi_pass</code> 行</li>
              <li>修改为：<code class="text-xs bg-slate-800 px-1 rounded text-emerald-400">fastcgi_pass {{ phpContainerName }}:9000;</code></li>
            </ol>
          </div>
          
          <div class="mt-4 flex gap-2">
            <button
              @click="copyNginxConfig"
              class="px-4 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-sm font-medium transition flex items-center gap-2"
            >
              <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
              </svg>
              复制配置代码
            </button>
            <button
              @click="showNginxHint = false"
              class="px-4 py-2 bg-slate-700 hover:bg-slate-600 rounded-lg text-sm font-medium transition"
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

    <div class="flex-1 overflow-y-auto pr-2 space-y-6">
      <!-- PHP Services -->
      <section class="bg-slate-900 border border-slate-800 rounded-xl p-6">
        <div class="flex justify-between items-center mb-4">
          <h2 class="text-lg font-bold">🐘 PHP 服务</h2>
          <button @click="addPhpVersion" class="text-sm px-3 py-1 bg-blue-600/20 text-blue-400 border border-blue-600/30 rounded-lg hover:bg-blue-600 hover:text-white transition">
            + 添加版本
          </button>
        </div>
        <div v-for="(php, idx) in phpServices" :key="idx" class="mb-6 p-4 bg-slate-800/50 border border-slate-700 rounded-lg">
          <div class="flex items-center gap-4 mb-3">
            <div class="flex-1">
              <label class="block text-xs text-slate-400 mb-1">PHP 版本</label>
              <select v-model="php.version" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500">
                <option v-for="v in phpVersions" :key="v" :value="v">PHP {{ v }}</option>
              </select>
            </div>
            <div class="w-32">
              <label class="block text-xs text-slate-400 mb-1">宿主机端口</label>
              <input v-model.number="php.host_port" type="number" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500" />
            </div>
            <button v-if="phpServices.length > 1" @click="removePhpVersion(idx)" class="mt-5 text-rose-400 hover:text-rose-300 text-sm">删除</button>
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
      <section class="bg-slate-900 border border-slate-800 rounded-xl p-6">
        <div class="flex justify-between items-center mb-4">
          <h2 class="text-lg font-bold">🐬 MySQL 服务</h2>
          <button @click="addMysqlVersion" class="text-sm px-3 py-1 bg-blue-600/20 text-blue-400 border border-blue-600/30 rounded-lg hover:bg-blue-600 hover:text-white transition">
            + 添加版本
          </button>
        </div>
        <div v-for="(mysql, idx) in mysqlServices" :key="idx" class="mb-4 p-4 bg-slate-800/50 border border-slate-700 rounded-lg">
          <div class="flex items-center gap-4">
            <div class="flex-1">
              <label class="block text-xs text-slate-400 mb-1">MySQL 版本</label>
              <select v-model="mysql.version" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500">
                <option v-for="v in mysqlVersions" :key="v" :value="v">MySQL {{ v }}</option>
              </select>
            </div>
            <div class="w-32">
              <label class="block text-xs text-slate-400 mb-1">宿主机端口</label>
              <input v-model.number="mysql.host_port" type="number" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500" />
            </div>
            <button v-if="mysqlServices.length > 1" @click="removeMysqlVersion(idx)" class="mt-5 text-rose-400 hover:text-rose-300 text-sm">删除</button>
          </div>
        </div>
      </section>

      <!-- Redis -->
      <section class="bg-slate-900 border border-slate-800 rounded-xl p-6">
        <div class="flex justify-between items-center mb-4">
          <h2 class="text-lg font-bold">🔴 Redis 服务</h2>
          <button @click="addRedisVersion" class="text-sm px-3 py-1 bg-blue-600/20 text-blue-400 border border-blue-600/30 rounded-lg hover:bg-blue-600 hover:text-white transition">
            + 添加版本
          </button>
        </div>
        <div v-for="(redis, idx) in redisServices" :key="idx" class="mb-4 p-4 bg-slate-800/50 border border-slate-700 rounded-lg">
          <div class="flex items-center gap-4">
            <div class="flex-1">
              <label class="block text-xs text-slate-400 mb-1">Redis 版本</label>
              <select v-model="redis.version" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500">
                <option v-for="v in redisVersions" :key="v" :value="v">Redis {{ v }}</option>
              </select>
            </div>
            <div class="w-32">
              <label class="block text-xs text-slate-400 mb-1">宿主机端口</label>
              <input v-model.number="redis.host_port" type="number" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500" />
            </div>
            <button @click="removeRedisVersion(idx)" class="mt-5 text-rose-400 hover:text-rose-300 text-sm">删除</button>
          </div>
        </div>
        <div v-if="redisServices.length === 0" class="text-center py-8 text-slate-500 text-sm">
          点击上方“+ 添加版本”按钮添加 Redis 服务
        </div>
      </section>

      <!-- Nginx -->
      <section class="bg-slate-900 border border-slate-800 rounded-xl p-6">
        <div class="flex justify-between items-center mb-4">
          <h2 class="text-lg font-bold">🚀 Nginx 服务</h2>
          <button @click="addNginxVersion" class="text-sm px-3 py-1 bg-blue-600/20 text-blue-400 border border-blue-600/30 rounded-lg hover:bg-blue-600 hover:text-white transition">
            + 添加版本
          </button>
        </div>
        <div v-for="(nginx, idx) in nginxServices" :key="idx" class="mb-4 p-4 bg-slate-800/50 border border-slate-700 rounded-lg">
          <div class="flex items-center gap-4">
            <div class="flex-1">
              <label class="block text-xs text-slate-400 mb-1">Nginx 版本</label>
              <select v-model="nginx.version" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500">
                <option v-for="v in nginxVersions" :key="v" :value="v">Nginx {{ v }}</option>
              </select>
            </div>
            <div class="w-32">
              <label class="block text-xs text-slate-400 mb-1">宿主机端口</label>
              <input v-model.number="nginx.host_port" type="number" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500" />
            </div>
            <button @click="removeNginxVersion(idx)" class="mt-5 text-rose-400 hover:text-rose-300 text-sm">删除</button>
          </div>
        </div>
        <div v-if="nginxServices.length === 0" class="text-center py-8 text-slate-500 text-sm">
          点击上方“+ 添加版本”按钮添加 Nginx 服务
        </div>
      </section>

      <!-- General Settings -->
      <section class="bg-slate-900 border border-slate-800 rounded-xl p-6">
        <h2 class="text-lg font-bold mb-4">⚙️ 通用设置</h2>
        <div class="flex gap-4">
          <div class="flex-1">
            <label class="block text-xs text-slate-400 mb-1">项目源码目录</label>
            <input v-model="sourceDir" type="text" placeholder="./www" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500" />
          </div>
          <div class="w-48">
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
    <div v-if="showPreviewModal" class="fixed inset-0 bg-black/70 flex items-center justify-center z-50" @click.self="showPreviewModal = false">
      <div class="bg-slate-900 border border-slate-700 rounded-xl max-w-6xl w-full mx-4 max-h-[90vh] flex flex-col">
        <div class="flex justify-between items-center p-6 border-b border-slate-700">
          <h2 class="text-xl font-bold">📄 配置预览</h2>
          <button @click="showPreviewModal = false" class="text-slate-400 hover:text-white text-2xl">&times;</button>
        </div>
        <div class="flex-1 overflow-y-auto p-6">
          <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
            <div>
              <div class="text-xs text-slate-400 mb-2 uppercase tracking-wider">.env</div>
              <pre class="bg-black/40 p-4 rounded-lg text-xs text-green-300/80 border border-slate-700 max-h-96 overflow-y-auto font-mono whitespace-pre-wrap">{{ previewEnv }}</pre>
            </div>
            <div>
              <div class="text-xs text-slate-400 mb-2 uppercase tracking-wider">docker-compose.yml</div>
              <pre class="bg-black/40 p-4 rounded-lg text-xs text-blue-300/80 border border-slate-700 max-h-96 overflow-y-auto font-mono whitespace-pre-wrap">{{ previewCompose }}</pre>
            </div>
          </div>
        </div>
        <div class="p-6 border-t border-slate-700 flex justify-end gap-3">
          <button @click="showPreviewModal = false" class="px-5 py-2 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg font-medium transition">
            关闭
          </button>
          <button @click="handleApply" :disabled="applying" class="px-5 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg font-medium transition disabled:opacity-50">
            {{ applying ? '应用中...' : '应用配置' }}
          </button>
        </div>
      </div>
    </div>

    <!-- 确认对话框 -->
    <div v-if="showConfirmDialog" class="fixed inset-0 z-[100] flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div class="bg-slate-900 border border-slate-700 rounded-xl shadow-2xl max-w-md w-full mx-4 animate-in fade-in zoom-in-95 duration-200">
        <!-- 标题栏 -->
        <div class="p-6 border-b border-slate-800">
          <div class="flex items-start gap-3">
            <div class="flex-shrink-0 w-10 h-10 rounded-full bg-yellow-500/10 flex items-center justify-center">
              <svg xmlns="http://www.w3.org/2000/svg" class="w-6 h-6 text-yellow-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
            </div>
            <div class="flex-1">
              <h3 class="text-lg font-semibold text-slate-100">{{ confirmTitle }}</h3>
              <p class="mt-2 text-sm text-slate-400 whitespace-pre-line">{{ confirmMessage }}</p>
            </div>
          </div>
        </div>
        
        <!-- 按钮栏 -->
        <div class="p-6 border-t border-slate-800 flex justify-end gap-3">
          <button 
            @click="handleConfirmDialog(false)" 
            class="px-5 py-2 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg font-medium transition text-slate-300"
          >
            取消
          </button>
          <button 
            @click="handleConfirmDialog(true)" 
            class="px-5 py-2 bg-red-600 hover:bg-red-700 rounded-lg font-medium transition text-white"
          >
            覆盖
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
