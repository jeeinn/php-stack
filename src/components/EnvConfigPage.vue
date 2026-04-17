<script setup lang="ts">
import { ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { ServiceEntry, EnvConfig } from '../types/env-config';

// Available versions
const phpVersions = ['5.6', '7.0', '7.1', '7.2', '7.3', '7.4', '8.0', '8.1', '8.2', '8.3', '8.4'];
const mysqlVersions = ['5.7', '8.0', '8.4'];
const redisVersions = ['6.2-alpine', '7.0-alpine', '7.2-alpine'];
const nginxVersions = ['1.24-alpine', '1.25-alpine', '1.26-alpine', '1.27-alpine'];

const commonExtensions = [
  'pdo_mysql', 'mysqli', 'mbstring', 'gd', 'curl', 'opcache', 'bcmath',
  'redis', 'xdebug', 'swoole', 'zip', 'pcntl', 'sockets', 'intl',
  'soap', 'imagick', 'mongodb', 'amqp', 'memcached',
];

// State
const phpServices = ref<ServiceEntry[]>([
  { service_type: 'PHP', version: '8.2', host_port: 9000, extensions: ['pdo_mysql', 'mysqli', 'mbstring', 'gd', 'curl', 'opcache'] },
]);
const mysqlEnabled = ref(true);
const mysqlVersion = ref('8.0');
const mysqlPort = ref(3306);

const redisEnabled = ref(true);
const redisVersion = ref('7.2-alpine');
const redisPort = ref(6379);

const nginxEnabled = ref(true);
const nginxVersion = ref('1.27-alpine');
const nginxPort = ref(80);

const sourceDir = ref('./www');
const timezone = ref('Asia/Shanghai');

const loading = ref(false);
const applying = ref(false);
const error = ref<string | null>(null);
const previewEnv = ref('');
const previewCompose = ref('');
const showPreview = ref(false);

// Port conflict detection
const allPorts = computed(() => {
  const ports: { service: string; port: number }[] = [];
  phpServices.value.forEach((s, i) => {
    ports.push({ service: `PHP ${s.version} (#${i + 1})`, port: s.host_port });
  });
  if (mysqlEnabled.value) ports.push({ service: 'MySQL', port: mysqlPort.value });
  if (redisEnabled.value) ports.push({ service: 'Redis', port: redisPort.value });
  if (nginxEnabled.value) ports.push({ service: 'Nginx', port: nginxPort.value });
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
  if (mysqlEnabled.value) {
    services.push({ service_type: 'MySQL', version: mysqlVersion.value, host_port: mysqlPort.value });
  }
  if (redisEnabled.value) {
    services.push({ service_type: 'Redis', version: redisVersion.value, host_port: redisPort.value });
  }
  if (nginxEnabled.value) {
    services.push({ service_type: 'Nginx', version: nginxVersion.value, host_port: nginxPort.value });
  }
  return { services, source_dir: sourceDir.value, timezone: timezone.value };
}

// Add PHP version
function addPhpVersion() {
  const usedVersions = phpServices.value.map(s => s.version);
  const available = phpVersions.filter(v => !usedVersions.includes(v));
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
  try {
    const config = buildConfig();
    const [envContent, composeContent] = await Promise.all([
      invoke<string>('generate_env_config', { config }),
      invoke<string>('preview_compose', { config }),
    ]);
    previewEnv.value = envContent;
    previewCompose.value = composeContent;
    showPreview.value = true;
  } catch (e) {
    error.value = e as string;
  } finally {
    loading.value = false;
  }
}

// Apply
async function handleApply() {
  if (portConflicts.value.length > 0) {
    error.value = portConflicts.value.join('\n');
    return;
  }
  applying.value = true;
  error.value = null;
  try {
    const config = buildConfig();
    await invoke('apply_env_config', { config });
    error.value = null;
    showPreview.value = false;
  } catch (e) {
    error.value = e as string;
  } finally {
    applying.value = false;
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
      </div>
    </header>

    <!-- Error / Conflict Alert -->
    <div v-if="error" class="mb-4 p-4 bg-rose-500/10 border border-rose-500/20 rounded-xl text-rose-400 text-sm">
      <pre class="whitespace-pre-wrap">{{ error }}</pre>
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
        <div class="flex items-center gap-3 mb-4">
          <label class="flex items-center gap-2 cursor-pointer">
            <input type="checkbox" v-model="mysqlEnabled" class="accent-blue-500" />
            <h2 class="text-lg font-bold">🐬 MySQL</h2>
          </label>
        </div>
        <div v-if="mysqlEnabled" class="flex gap-4">
          <div class="flex-1">
            <label class="block text-xs text-slate-400 mb-1">版本</label>
            <select v-model="mysqlVersion" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500">
              <option v-for="v in mysqlVersions" :key="v" :value="v">MySQL {{ v }}</option>
            </select>
          </div>
          <div class="w-32">
            <label class="block text-xs text-slate-400 mb-1">宿主机端口</label>
            <input v-model.number="mysqlPort" type="number" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500" />
          </div>
        </div>
      </section>

      <!-- Redis -->
      <section class="bg-slate-900 border border-slate-800 rounded-xl p-6">
        <div class="flex items-center gap-3 mb-4">
          <label class="flex items-center gap-2 cursor-pointer">
            <input type="checkbox" v-model="redisEnabled" class="accent-blue-500" />
            <h2 class="text-lg font-bold">🔴 Redis</h2>
          </label>
        </div>
        <div v-if="redisEnabled" class="flex gap-4">
          <div class="flex-1">
            <label class="block text-xs text-slate-400 mb-1">版本</label>
            <select v-model="redisVersion" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500">
              <option v-for="v in redisVersions" :key="v" :value="v">Redis {{ v }}</option>
            </select>
          </div>
          <div class="w-32">
            <label class="block text-xs text-slate-400 mb-1">宿主机端口</label>
            <input v-model.number="redisPort" type="number" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500" />
          </div>
        </div>
      </section>

      <!-- Nginx -->
      <section class="bg-slate-900 border border-slate-800 rounded-xl p-6">
        <div class="flex items-center gap-3 mb-4">
          <label class="flex items-center gap-2 cursor-pointer">
            <input type="checkbox" v-model="nginxEnabled" class="accent-blue-500" />
            <h2 class="text-lg font-bold">🚀 Nginx</h2>
          </label>
        </div>
        <div v-if="nginxEnabled" class="flex gap-4">
          <div class="flex-1">
            <label class="block text-xs text-slate-400 mb-1">版本</label>
            <select v-model="nginxVersion" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500">
              <option v-for="v in nginxVersions" :key="v" :value="v">Nginx {{ v }}</option>
            </select>
          </div>
          <div class="w-32">
            <label class="block text-xs text-slate-400 mb-1">宿主机端口</label>
            <input v-model.number="nginxPort" type="number" class="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500" />
          </div>
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

      <!-- Preview Panel -->
      <section v-if="showPreview" class="bg-slate-900 border border-slate-800 rounded-xl p-6">
        <h2 class="text-lg font-bold mb-4">📄 配置预览</h2>
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <div>
            <div class="text-xs text-slate-400 mb-2 uppercase tracking-wider">.env</div>
            <pre class="bg-black/40 p-4 rounded-lg text-xs text-green-300/80 border border-slate-700 max-h-80 overflow-y-auto font-mono whitespace-pre-wrap">{{ previewEnv }}</pre>
          </div>
          <div>
            <div class="text-xs text-slate-400 mb-2 uppercase tracking-wider">docker-compose.yml</div>
            <pre class="bg-black/40 p-4 rounded-lg text-xs text-blue-300/80 border border-slate-700 max-h-80 overflow-y-auto font-mono whitespace-pre-wrap">{{ previewCompose }}</pre>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>
