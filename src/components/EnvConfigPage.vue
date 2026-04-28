<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-shell';
import type { ServiceEntry, EnvConfig, VersionInfo } from '../types/env-config';
import { showToast } from '../composables/useToast';
import { showConfirm } from '../composables/useConfirmDialog';
import CustomSelect from './CustomSelect.vue';

const { t } = useI18n();

const emit = defineEmits<{
  (e: 'request-switch-tab', tabName: string): void;
}>();

// Available versions (将从后端动态加载)
const phpVersions = ref<VersionInfo[]>([]);
const mysqlVersions = ref<VersionInfo[]>([]);
const redisVersions = ref<VersionInfo[]>([]);
const nginxVersions = ref<VersionInfo[]>([]);

// PHP 扩展预设列表（扁平化）
const commonExtensions = [
  'pdo_mysql', 'mysqli', 'mbstring', 'gd', 'curl', 'opcache', 'zip', 'bcmath', 'intl',
  'pdo_pgsql', 'pdo_sqlite', 'mongodb', 'redis', 'memcached', 'amqp',
  'swoole', 'openswoole', 'parallel',
  'xdebug', 'blackfire', 'tidy', 'soap',
  'imagick', 'exif', 'pcntl', 'sockets'
];

const customExtInput = ref('');
const isExtensionsPanelOpen = ref(false);

// Common timezones for selection
const commonTimezones = [
  { value: 'Asia/Shanghai', labelKey: 'envConfig.timezone.shanghai' },
  { value: 'Asia/Hong_Kong', labelKey: 'envConfig.timezone.hongkong' },
  { value: 'Asia/Tokyo', labelKey: 'envConfig.timezone.tokyo' },
  { value: 'Asia/Singapore', labelKey: 'envConfig.timezone.singapore' },
  { value: 'Asia/Dubai', labelKey: 'envConfig.timezone.dubai' },
  { value: 'Europe/London', labelKey: 'envConfig.timezone.london' },
  { value: 'Europe/Paris', labelKey: 'envConfig.timezone.paris' },
  { value: 'Europe/Berlin', labelKey: 'envConfig.timezone.berlin' },
  { value: 'America/New_York', labelKey: 'envConfig.timezone.newYork' },
  { value: 'America/Los_Angeles', labelKey: 'envConfig.timezone.losAngeles' },
  { value: 'America/Chicago', labelKey: 'envConfig.timezone.chicago' },
  { value: 'Australia/Sydney', labelKey: 'envConfig.timezone.sydney' },
  { value: 'UTC', labelKey: 'envConfig.timezone.utc' },
];

// Computed options for selects
const phpVersionOptions = computed(() => 
  phpVersions.value.map(v => ({
    value: v.id,
    label: `${v.display_name} → ${v.image_tag}${v.eol ? ' (EOL)' : ''}`,
    disabled: false,
  }))
);

const mysqlVersionOptions = computed(() => 
  mysqlVersions.value.map(v => ({
    value: v.id,
    label: `${v.display_name} → ${v.image_tag}${v.eol ? ' (EOL)' : ''}`,
    disabled: false,
  }))
);

const redisVersionOptions = computed(() => 
  redisVersions.value.map(v => ({
    value: v.id,
    label: `${v.display_name} → ${v.image_tag}${v.eol ? ' (EOL)' : ''}`,
    disabled: false,
  }))
);

const nginxVersionOptions = computed(() => 
  nginxVersions.value.map(v => ({
    value: v.id,
    label: `${v.display_name} → ${v.image_tag}${v.eol ? ' (EOL)' : ''}`,
    disabled: false,
  }))
);

const timezoneOptions = computed(() => [
  ...commonTimezones.map(tz => ({ value: tz.value, label: t(tz.labelKey) })),
  ...(showCustomTimezoneInput.value && customTimezone.value ? [{ value: customTimezone.value, label: t('envConfig.general.customTimezoneLabel', { tz: customTimezone.value }) }] : []),
  { value: '__custom__', label: t('envConfig.general.customTimezone') },
]);

// State
const phpServices = ref<ServiceEntry[]>([]);
const mysqlServices = ref<ServiceEntry[]>([]);
const redisServices = ref<ServiceEntry[]>([]);
const nginxServices = ref<ServiceEntry[]>([]);

const sourceDir = ref('./www');
const timezone = ref('Asia/Shanghai');
const customTimezone = ref('');
const showCustomTimezoneInput = ref(false);
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
      workspacePath.value = t('workspace.status.notConfigured');
    }
  } catch (e) {
    workspacePath.value = t('workspace.status.loadFailed');
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
    
    // 提取版本信息列表（包含 id、display_name、image_tag、service_dir 等完整信息）
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
      { id: 'php56', display_name: 'PHP 5.6', image_tag: 'php:5.6-fpm', service_dir: 'php56', default_port: 9000, show_port: false, eol: true },
      { id: 'php74', display_name: 'PHP 7.4', image_tag: 'php:7.4-fpm', service_dir: 'php74', default_port: 9000, show_port: false, eol: true },
      { id: 'php80', display_name: 'PHP 8.0', image_tag: 'php:8.0-fpm', service_dir: 'php80', default_port: 9000, show_port: false, eol: true },
      { id: 'php81', display_name: 'PHP 8.1', image_tag: 'php:8.1-fpm', service_dir: 'php81', default_port: 9000, show_port: false, eol: false },
      { id: 'php82', display_name: 'PHP 8.2', image_tag: 'php:8.2-fpm', service_dir: 'php82', default_port: 9000, show_port: false, eol: false },
      { id: 'php83', display_name: 'PHP 8.3', image_tag: 'php:8.3-fpm', service_dir: 'php83', default_port: 9000, show_port: false, eol: false },
      { id: 'php84', display_name: 'PHP 8.4', image_tag: 'php:8.4-fpm', service_dir: 'php84', default_port: 9000, show_port: false, eol: false },
    ];
    mysqlVersions.value = [
      { id: 'mysql57', display_name: 'MySQL 5.7', image_tag: 'mysql:5.7', service_dir: 'mysql57', default_port: 3306, show_port: true, eol: true },
      { id: 'mysql80', display_name: 'MySQL 8.0', image_tag: 'mysql:8.0', service_dir: 'mysql80', default_port: 3306, show_port: true, eol: false },
      { id: 'mysql84', display_name: 'MySQL 8.4 LTS', image_tag: 'mysql:8.4', service_dir: 'mysql84', default_port: 3306, show_port: true, eol: false },
    ];
    redisVersions.value = [
      { id: 'redis62', display_name: 'Redis 6.2', image_tag: 'redis:6.2-alpine', service_dir: 'redis62', default_port: 6379, show_port: true, eol: true },
      { id: 'redis70', display_name: 'Redis 7.0', image_tag: 'redis:7.0-alpine', service_dir: 'redis70', default_port: 6379, show_port: true, eol: false },
      { id: 'redis72', display_name: 'Redis 7.2', image_tag: 'redis:7.2-alpine', service_dir: 'redis72', default_port: 6379, show_port: true, eol: false },
      { id: 'redis82', display_name: 'Redis 8.2', image_tag: 'redis:8.2-alpine', service_dir: 'redis82', default_port: 6379, show_port: true, eol: false },
    ];
    nginxVersions.value = [
      { id: 'nginx124', display_name: 'Nginx 1.24', image_tag: 'nginx:1.24-alpine', service_dir: 'nginx124', default_port: 80, show_port: true, eol: true },
      { id: 'nginx125', display_name: 'Nginx 1.25', image_tag: 'nginx:1.25-alpine', service_dir: 'nginx125', default_port: 80, show_port: true, eol: false },
      { id: 'nginx127', display_name: 'Nginx 1.27', image_tag: 'nginx:1.27-alpine', service_dir: 'nginx127', default_port: 80, show_port: true, eol: false },
    ];
  }
}

// 错误信息格式化
function formatErrorMessage(error: any): string {
  const errorMsg = String(error);
  
  if (errorMsg.includes('Docker') || errorMsg.includes('docker')) {
    if (errorMsg.includes('not running') || errorMsg.includes('unavailable')) {
      return t('envConfig.error.dockerNotRunning');
    }
    if (errorMsg.includes('permission')) {
      return t('envConfig.error.permissionDenied');
    }
  }
  
  if (errorMsg.includes('端口') || errorMsg.includes('port')) {
    return t('envConfig.error.portConflict', { error: errorMsg });
  }
  
  if (errorMsg.includes('读取') || errorMsg.includes('read')) {
    return t('envConfig.error.readFailed');
  }
  if (errorMsg.includes('写入') || errorMsg.includes('write')) {
    return t('envConfig.error.writeFailed');
  }
  
  if (errorMsg.includes('解析') || errorMsg.includes('parse')) {
    return t('envConfig.error.parseFailed');
  }
  
  return t('envConfig.error.default', { error: errorMsg });
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
        } else if (s.service_type === 'MySQL') {
          mysqlSvcs.push({ ...s });
        } else if (s.service_type === 'Redis') {
          redisSvcs.push({ ...s });
        } else if (s.service_type === 'Nginx') {
          nginxSvcs.push({ ...s });
        }
      });
      
      console.log('[EnvConfig] PHP 服务:', phpSvcs);
      console.log('[EnvConfig] MySQL 服务:', mysqlSvcs);
      console.log('[EnvConfig] Redis 服务:', redisSvcs);
      console.log('[EnvConfig] Nginx 服务:', nginxSvcs);
      
      phpServices.value = phpSvcs.length > 0 ? phpSvcs : [{
        service_type: 'PHP',
        version: 'php82',
        host_port: 9000,
        extensions: ['pdo_mysql', 'mysqli', 'mbstring', 'gd', 'curl', 'opcache'],
      }];
      
      mysqlServices.value = mysqlSvcs.length > 0 ? mysqlSvcs : [{
        service_type: 'MySQL',
        version: 'mysql80',
        host_port: 3306,
      }];
      
      redisServices.value = redisSvcs.length > 0 ? redisSvcs : [];
      nginxServices.value = nginxSvcs.length > 0 ? nginxSvcs : [];
      
      sourceDir.value = config.source_dir;
      timezone.value = config.timezone;
      
      // Check if timezone is in common list
      const isInCommonList = commonTimezones.some(tz => tz.value === config.timezone);
      if (!isInCommonList && config.timezone) {
        customTimezone.value = config.timezone;
        showCustomTimezoneInput.value = true;
      }
      
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
        version: 'php82',
        host_port: 9000,
        extensions: ['pdo_mysql', 'mysqli', 'mbstring', 'gd', 'curl', 'opcache'],
      }];
      mysqlServices.value = [{
        service_type: 'MySQL',
        version: 'mysql80',
        host_port: 3306,
      }];
    }
  } catch (e) {
    console.error('[EnvConfig] 加载配置失败:', e);
    // Use defaults
    phpServices.value = [{
      service_type: 'PHP',
      version: 'php82',
      host_port: 9000,
      extensions: ['pdo_mysql', 'mysqli', 'mbstring', 'gd', 'curl', 'opcache'],
    }];
    mysqlServices.value = [{
      service_type: 'MySQL',
      version: 'mysql80',
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
      conflicts.push(t('envConfig.portConflict.detail', { port, service1: seen.get(port), service2: service }));
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
  const available = phpVersions.value.filter(v => !usedVersions.includes(v.id));
  if (available.length === 0) return;
  phpServices.value.push({
    service_type: 'PHP',
    version: available[0].id,
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
  const available = mysqlVersions.value.filter(v => !usedVersions.includes(v.id));
  if (available.length === 0) return;
  mysqlServices.value.push({
    service_type: 'MySQL',
    version: available[0].id,
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
  const available = redisVersions.value.filter(v => !usedVersions.includes(v.id));
  if (available.length === 0) return;
  redisServices.value.push({
    service_type: 'Redis',
    version: available[0].id,
    host_port: 6379 + redisServices.value.length,
  });
}

function removeRedisVersion(index: number) {
  redisServices.value.splice(index, 1);
}

// Add Nginx version
function addNginxVersion() {
  const usedVersions = nginxServices.value.map(s => s.version);
  const available = nginxVersions.value.filter(v => !usedVersions.includes(v.id));
  if (available.length === 0) return;
  nginxServices.value.push({
    service_type: 'Nginx',
    version: available[0].id,
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

function syncCustomExtensions(phpIndex: number) {
  const service = phpServices.value[phpIndex];
  if (!service.extensions) service.extensions = [];
  
  // 获取当前已选的预设扩展
  const presetExts = service.extensions.filter(e => commonExtensions.includes(e));
  
  // 解析用户输入的自定义扩展
  const customExts = customExtInput.value
    .split(/[,\s]+/)
    .map(s => s.trim())
    .filter(s => s.length > 0 && !commonExtensions.includes(s));
  
  // 合并并去重
  service.extensions = [...new Set([...presetExts, ...customExts])];
}

function handleCustomTimezoneChange() {
  if (customTimezone.value.trim()) {
    timezone.value = customTimezone.value.trim();
  }
}

function handleTimezoneChange(value: string) {
  if (value === '__custom__') {
    showCustomTimezoneInput.value = true;
    customTimezone.value = '';
  } else {
    showCustomTimezoneInput.value = false;
    timezone.value = value;
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
        title: t('envConfig.confirmOverwrite.title'),
        message: t('envConfig.confirmOverwrite.message', { files: fileList }),
        confirmText: t('envConfig.confirmOverwrite.confirm'),
        cancelText: t('common.cancel'),
        type: 'warning',
        checkboxLabel: t('envConfig.confirmOverwrite.backupLabel'),
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
    let successMsg = t('envConfig.toast.applySuccess', { 
        location: import.meta.env.DEV ? t('envConfig.toast.applySuccessDev') : t('envConfig.toast.applySuccessProd')
      });
    
    if (backedUpFiles && backedUpFiles.length > 0) {
      successMsg += t('envConfig.toast.backedUp', { count: backedUpFiles.length, files: backedUpFiles.map(f => '• ' + f).join('\n') });
    } else if (enableBackup) {
      successMsg += t('envConfig.toast.backupPartialFail');
    }
    
    showToast(successMsg, 'success', 6000);
    showPreviewModal.value = false;
    
    // 更新 .env 文件存在状态
    hasEnvFile.value = true;
    
    // 检查是否同时启用了 PHP 和 Nginx
    const hasPHP = phpServices.value.length > 0;
    const hasNginx = nginxServices.value.length > 0;
    
    if (hasPHP && hasNginx) {
      // 获取所有 PHP 服务的容器地址（容器名:端口）— 直接使用 service_dir
      phpContainerNames.value = phpServices.value.map(service => {
        const versionInfo = phpVersions.value.find(v => v.id === service.version);
        const serviceDir = versionInfo ? versionInfo.service_dir : service.version;
        return `ps-${serviceDir}:9000`;
      });
      
      // 获取所有 Nginx 服务的信息 — 直接使用 service_dir
      nginxServicesList.value = nginxServices.value.map(service => {
        const versionInfo = nginxVersions.value.find(v => v.id === service.version);
        const serviceDir = versionInfo ? versionInfo.service_dir : service.version;
        const displayName = versionInfo ? versionInfo.display_name : service.version;
        return {
          name: serviceDir,
          version: displayName,
          port: service.host_port
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
async function openNginxConfigDir(serviceDir?: string) {
  try {
    // 如果没有指定服务目录，默认打开第一个 Nginx 的配置目录
    const targetDir = serviceDir || (nginxServicesList.value.length > 0 ? nginxServicesList.value[0].name : 'nginx127');
    await invoke('open_service_config', { serviceName: targetDir });
    showToast(t('envConfig.toast.nginxConfigOpened', { dir: targetDir }), 'success');
  } catch (e) {
    console.error('打开目录失败:', e);
    const targetDir = serviceDir || (nginxServicesList.value.length > 0 ? nginxServicesList.value[0].name : 'nginx127');
    showToast(t('envConfig.toast.nginxConfigFailed', { dir: targetDir }), 'error');
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
    showToast(t('envConfig.toast.startSuccess', { result }), 'success', 5000);
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
  <div class="flex-1 flex flex-col overflow-hidden bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-200 transition-colors duration-300">
    <header class="flex flex-col lg:flex-row justify-between items-start lg:items-center gap-4 mb-6">
      <div>
        <h1 class="text-2xl sm:text-3xl font-bold text-slate-900 dark:text-slate-100">{{ $t('envConfig.title') }}</h1>
        <p class="text-slate-500 dark:text-slate-400 text-xs sm:text-sm mt-1">{{ $t('envConfig.subtitle') }}</p>
      </div>
      <div class="flex flex-col sm:flex-row gap-3 w-full lg:w-auto">
        <button
          @click="handlePreview"
          :disabled="loading"
          class="w-full sm:w-auto px-5 py-2 bg-slate-800 dark:bg-slate-800 hover:bg-slate-700 dark:hover:bg-slate-700 border border-slate-700 dark:border-slate-700 text-white dark:text-slate-300 rounded-lg font-medium transition disabled:opacity-50"
        >
          {{ loading ? $t('envConfig.previewing') : $t('envConfig.preview') }}
        </button>
        <button
          @click="handleApply"
          :disabled="applying || portConflicts.length > 0"
          class="w-full sm:w-auto px-5 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg font-medium transition disabled:opacity-50"
        >
          {{ applying ? $t('envConfig.applying') : $t('envConfig.apply') }}
        </button>
        <button
          @click="handleStart"
          :disabled="starting || !hasEnvFile"
          class="w-full sm:w-auto px-5 py-2 bg-green-600 hover:bg-green-700 rounded-lg font-medium transition disabled:opacity-50 disabled:cursor-not-allowed"
          :title="!hasEnvFile ? $t('envConfig.startTooltip') : ''"
        >
          {{ starting ? $t('envConfig.startingEnv') : $t('envConfig.startEnv') }}
        </button>
      </div>
    </header>
    
    <!-- Nginx 配置提示 -->
    <div v-if="showNginxHint" class="mb-4 p-4 sm:p-5 bg-blue-50 dark:bg-blue-950/30 border border-blue-200 dark:border-blue-800 rounded-xl">
      <div class="flex flex-col sm:flex-row items-start gap-3">
        <div class="flex-shrink-0">
          <svg xmlns="http://www.w3.org/2000/svg" class="w-6 h-6 text-blue-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        </div>
        <div class="flex-1">
          <h3 class="text-base font-semibold text-blue-600 dark:text-blue-400 mb-2">{{ $t('envConfig.nginxHint.title') }}</h3>
          <p class="text-sm text-slate-700 dark:text-slate-300 mb-3">
            {{ $t('envConfig.nginxHint.description') }}
          </p>
          
          <div class="bg-slate-100 dark:bg-slate-800 rounded-lg p-3 mb-3 border border-slate-200 dark:border-slate-700">
            <p class="text-xs text-slate-400 mb-2">{{ $t('envConfig.nginxHint.phpAddresses') }}</p>
            <div class="space-y-1">
              <div v-for="(name, index) in phpContainerNames" :key="index" class="flex items-center gap-2">
                <span class="text-xs text-slate-500 font-mono">{{ index + 1 }}.</span>
                <code class="text-sm text-emerald-400 font-mono">{{ name }}</code>
              </div>
            </div>
          </div>
          
          <!-- 多 Nginx 版本提示 -->
          <div v-if="nginxServicesList.length > 1" class="bg-amber-100 dark:bg-amber-950/30 rounded-lg p-3 mb-3 border border-amber-200 dark:border-amber-800">
            <p class="text-xs text-amber-700 dark:text-amber-300 mb-2">{{ $t('envConfig.nginxHint.multiNginx') }}</p>
            <div class="space-y-2">
              <div v-for="(nginx, index) in nginxServicesList" :key="index" class="flex flex-col sm:flex-row sm:items-center gap-2 text-sm">
                <div class="flex items-center gap-2">
                  <span class="text-xs text-slate-500 dark:text-slate-500 font-mono">{{ index + 1 }}.</span>
                  <code class="text-sm text-blue-600 dark:text-blue-400 font-mono">{{ nginx.name }}</code>
                  <span class="text-xs text-slate-500 dark:text-slate-500">({{ nginx.version }})</span>
                  <span v-if="nginx.port" class="text-xs text-slate-500 dark:text-slate-500">- {{ $t('envConfig.nginxHint.port', { port: nginx.port }) }}</span>
                </div>
                <button
                  @click="openNginxConfigDir(nginx.name)"
                  class="sm:ml-auto px-3 py-1 bg-blue-600/20 hover:bg-blue-600/30 text-blue-400 rounded text-xs transition border border-blue-600/30 whitespace-nowrap"
                >
                  {{ $t('envConfig.nginxHint.openConfigDir') }}
                </button>
              </div>
            </div>
          </div>
          
          <div class="space-y-2 text-sm text-slate-700 dark:text-slate-300">
            <p><strong class="text-blue-600 dark:text-blue-400">{{ $t('envConfig.nginxHint.configSteps') }}</strong></p>
            <ol class="list-decimal list-inside space-y-1 ml-2 text-slate-600 dark:text-slate-400">
              <li v-if="nginxServicesList.length === 1">
                {{ $t('envConfig.nginxHint.step1Single', { path: `services/${nginxServicesList[0].name}/conf.d/default.conf` }) }}
              </li>
              <li v-else>
                {{ $t('envConfig.nginxHint.step1Multi') }}
              </li>
              <li>{{ $t('envConfig.nginxHint.step2', { directive: 'fastcgi_pass', default: 'php:9000' }) }}</li>
              <li>{{ $t('envConfig.nginxHint.step3', { example: 'fastcgi_pass [container_address];', hint: 'ps-php85:9000' }) }}</li>
              <li class="text-xs text-slate-500 dark:text-slate-500 mt-1">{{ $t('envConfig.nginxHint.step4') }}</li>
            </ol>
          </div>
          
          <div class="mt-4 flex flex-col sm:flex-row gap-2">
            <button
              v-if="nginxServicesList.length === 1"
              @click="openNginxConfigDir()"
              class="w-full sm:w-auto px-4 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-sm font-medium transition flex items-center justify-center gap-2 text-white"
            >
              <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
              </svg>
              {{ $t('envConfig.nginxHint.openConfigDir') }}
            </button>
            <button
              @click="showNginxHint = false"
              class="w-full sm:w-auto px-4 py-2 bg-slate-200 dark:bg-slate-700 hover:bg-slate-300 dark:hover:bg-slate-600 rounded-lg text-sm font-medium transition text-slate-700 dark:text-slate-300"
            >
              {{ $t('envConfig.nginxHint.dismiss') }}
            </button>
          </div>
        </div>
      </div>
    </div>
    
    <div v-if="portConflicts.length > 0" class="mb-4 p-4 bg-amber-500/10 dark:bg-amber-500/10 border border-amber-500/20 rounded-xl text-amber-600 dark:text-amber-400 text-sm">
      <div class="font-bold mb-1">{{ $t('envConfig.portConflict.title') }}</div>
      <div v-for="c in portConflicts" :key="c">{{ c }}</div>
    </div>

    <div class="flex-1 overflow-y-auto pr-1 sm:pr-2 space-y-4 sm:space-y-6">
      <!-- PHP Services -->
      <section class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-4 sm:p-6">
        <div class="flex justify-between items-center mb-4">
          <h2 class="text-lg font-bold text-slate-900 dark:text-slate-200">{{ $t('envConfig.php.title') }}</h2>
          <button @click="addPhpVersion" class="text-sm px-3 py-1 bg-blue-600/20 text-blue-600 dark:text-blue-400 border border-blue-600/30 rounded-lg hover:bg-blue-600 hover:text-white transition">
            {{ $t('envConfig.addVersion') }}
          </button>
        </div>
        <div v-for="(php, idx) in phpServices" :key="idx" class="mb-4 sm:mb-6 p-3 sm:p-4 bg-slate-50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded-lg">
          <div class="flex flex-col sm:flex-row items-start sm:items-center gap-3 sm:gap-4 mb-3">
            <div class="flex-1 w-full sm:w-auto">
              <label class="block text-xs text-slate-600 dark:text-slate-400 mb-1">
                {{ $t('envConfig.php.version') }}
              </label>
              <CustomSelect 
                v-model="php.version" 
                :options="phpVersionOptions"
                :placeholder="$t('envConfig.php.versionPlaceholder')" 
              />
            </div>
            <button v-if="phpServices.length > 1" @click="removePhpVersion(idx)" class="w-full sm:w-auto mt-2 sm:mt-5 text-rose-600 dark:text-rose-400 hover:text-rose-700 dark:hover:text-rose-300 text-sm">{{ $t('envConfig.remove') }}</button>
          </div>
          <div>
            <label class="block text-xs text-slate-600 dark:text-slate-400 mb-2">{{ $t('envConfig.php.extensions') }}</label>
            
            <!-- 统一折叠面板 -->
            <div class="border border-slate-200 dark:border-slate-700/50 rounded-lg overflow-hidden">
              <button 
                @click="isExtensionsPanelOpen = !isExtensionsPanelOpen"
                class="w-full flex justify-between items-center px-3 py-2 bg-slate-50 dark:bg-slate-800/50 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors text-left"
              >
                <span class="text-xs font-medium text-slate-700 dark:text-slate-300">{{ $t('envConfig.php.extensionsPanel') }}</span>
                <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5 text-slate-500 transition-transform duration-200" :class="{ 'rotate-180': isExtensionsPanelOpen }" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7" />
                </svg>
              </button>
              
              <div v-show="isExtensionsPanelOpen" class="p-3 bg-slate-50 dark:bg-slate-900/50 border-t border-slate-200 dark:border-slate-700/50 space-y-3">
                <!-- 平铺扩展列表 -->
                <div class="max-h-64 overflow-y-auto pr-1 custom-scrollbar">
                  <div class="flex flex-wrap gap-2">
                    <label
                      v-for="ext in commonExtensions"
                      :key="ext"
                      class="flex items-center gap-1.5 text-[11px] px-2 py-1 rounded cursor-pointer transition select-none"
                      :class="php.extensions?.includes(ext) ? 'bg-blue-600/20 text-blue-600 dark:text-blue-400 border border-blue-500/30' : 'bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-500 border border-slate-300 dark:border-slate-700 hover:border-slate-400 dark:hover:border-slate-600'"
                    >
                      <input type="checkbox" :checked="php.extensions?.includes(ext)" @change="toggleExtension(idx, ext)" class="hidden" />
                      {{ ext }}
                    </label>
                  </div>
                </div>

                <!-- 自定义扩展输入区 -->
                <div class="pt-3 border-t border-slate-200 dark:border-slate-700/50">
                  <label class="block text-[10px] font-medium text-emerald-600 dark:text-emerald-400 mb-1.5">{{ $t('envConfig.php.customExtensions') }}</label>
                  <input 
                    v-model="customExtInput" 
                    @blur="syncCustomExtensions(idx)"
                    :placeholder="$t('envConfig.php.customExtPlaceholder')"  
                    class="w-full bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-1.5 text-xs text-emerald-600 dark:text-emerald-400 font-mono outline-none focus:ring-2 focus:ring-emerald-500/50"
                  />
                  <div class="flex items-center justify-between mt-1.5">
                    <p class="text-[10px] text-slate-500 dark:text-slate-500">{{ $t('envConfig.php.customExtHint') }}</p>
                    <button 
                      @click="open('https://github.com/mlocati/docker-php-extension-installer#supported-php-extensions')"
                      class="text-[10px] text-blue-600 dark:text-blue-400 hover:text-blue-700 dark:hover:text-blue-300 flex items-center gap-1 transition-colors cursor-pointer"
                    >
                      <span>{{ $t('envConfig.php.viewSupported') }}</span>
                      <svg xmlns="http://www.w3.org/2000/svg" class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
                      </svg>
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>

      <!-- MySQL -->
      <section class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-4 sm:p-6">
        <div class="flex justify-between items-center mb-4">
          <h2 class="text-lg font-bold text-slate-900 dark:text-slate-200">{{ $t('envConfig.mysql.title') }}</h2>
          <button @click="addMysqlVersion" class="text-sm px-3 py-1 bg-blue-600/20 text-blue-400 border border-blue-600/30 rounded-lg hover:bg-blue-600 hover:text-white transition">
            {{ $t('envConfig.addVersion') }}
          </button>
        </div>
        <div v-for="(mysql, idx) in mysqlServices" :key="idx" class="mb-3 sm:mb-4 p-3 sm:p-4 bg-slate-50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded-lg">
          <div class="flex flex-col sm:flex-row items-start sm:items-center gap-3 sm:gap-4">
            <div class="flex-1 w-full sm:w-auto">
              <label class="block text-xs text-slate-600 dark:text-slate-400 mb-1">
                {{ $t('envConfig.mysql.version') }}
              </label>
              <CustomSelect 
                v-model="mysql.version" 
                :options="mysqlVersionOptions"
                :placeholder="$t('envConfig.mysql.versionPlaceholder')" 
              />
            </div>
            <div class="w-full sm:w-32" v-if="mysqlVersions.find(v => v.id === mysql.version)?.show_port !== false">
              <label class="block text-xs text-slate-600 dark:text-slate-400 mb-1">{{ $t('envConfig.hostPort') }}</label>
              <input v-model.number="mysql.host_port" type="number" class="w-full bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-900 dark:text-slate-200 outline-none focus:ring-2 focus:ring-blue-500" />
            </div>
            <button v-if="mysqlServices.length > 1" @click="removeMysqlVersion(idx)" class="w-full sm:w-auto mt-2 sm:mt-5 text-rose-400 hover:text-rose-300 text-sm">{{ $t('envConfig.remove') }}</button>
          </div>
        </div>
        
        <!-- MySQL Root 密码配置 -->
        <div class="mt-4 p-3 sm:p-4 bg-slate-50 dark:bg-slate-800/30 border border-slate-200 dark:border-slate-700/50 rounded-lg">
          <div class="flex flex-col sm:flex-row items-start sm:items-center gap-3 sm:gap-4">
            <div class="flex-1 w-full sm:w-64">
              <label class="block text-xs text-slate-600 dark:text-slate-400 mb-1">{{ $t('envConfig.mysql.rootPassword') }}</label>
              <input 
                v-model="mysqlRootPassword" 
                type="password" 
                placeholder="root"
                class="w-full bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-900 dark:text-slate-200 outline-none focus:ring-2 focus:ring-blue-500" 
              />
            </div>
            <div class="flex-1">
              <p class="text-xs text-slate-500 dark:text-slate-500">{{ $t('envConfig.mysql.rootPasswordHint') }}</p>
            </div>
          </div>
        </div>
      </section>

      <!-- Redis -->
      <section class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-4 sm:p-6">
        <div class="flex justify-between items-center mb-4">
          <h2 class="text-lg font-bold text-slate-900 dark:text-slate-200">{{ $t('envConfig.redis.title') }}</h2>
          <button @click="addRedisVersion" class="text-sm px-3 py-1 bg-blue-600/20 text-blue-400 border border-blue-600/30 rounded-lg hover:bg-blue-600 hover:text-white transition">
            {{ $t('envConfig.addVersion') }}
          </button>
        </div>
        <div v-for="(redis, idx) in redisServices" :key="idx" class="mb-3 sm:mb-4 p-3 sm:p-4 bg-slate-50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded-lg">
          <div class="flex flex-col sm:flex-row items-start sm:items-center gap-3 sm:gap-4">
            <div class="flex-1 w-full sm:w-auto">
              <label class="block text-xs text-slate-600 dark:text-slate-400 mb-1">
                {{ $t('envConfig.redis.version') }}
              </label>
              <CustomSelect 
                v-model="redis.version" 
                :options="redisVersionOptions"
                :placeholder="$t('envConfig.redis.versionPlaceholder')" 
              />
            </div>
            <div class="w-full sm:w-32" v-if="redisVersions.find(v => v.id === redis.version)?.show_port !== false">
              <label class="block text-xs text-slate-600 dark:text-slate-400 mb-1">{{ $t('envConfig.hostPort') }}</label>
              <input v-model.number="redis.host_port" type="number" class="w-full bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-900 dark:text-slate-200 outline-none focus:ring-2 focus:ring-blue-500" />
            </div>
            <button @click="removeRedisVersion(idx)" class="w-full sm:w-auto mt-2 sm:mt-5 text-rose-400 hover:text-rose-300 text-sm">{{ $t('envConfig.remove') }}</button>
          </div>
        </div>
        <div v-if="redisServices.length === 0" class="text-center py-8 text-slate-500 dark:text-slate-500 text-sm">
          点击上方“{{ $t('envConfig.addVersion') }}”按钮添加 Redis 服务
        </div>
      </section>

      <!-- Nginx -->
      <section class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-4 sm:p-6">
        <div class="flex justify-between items-center mb-4">
          <h2 class="text-lg font-bold text-slate-900 dark:text-slate-200">{{ $t('envConfig.nginx.title') }}</h2>
          <button @click="addNginxVersion" class="text-sm px-3 py-1 bg-blue-600/20 text-blue-400 border border-blue-600/30 rounded-lg hover:bg-blue-600 hover:text-white transition">
            {{ $t('envConfig.addVersion') }}
          </button>
        </div>
        <div v-for="(nginx, idx) in nginxServices" :key="idx" class="mb-3 sm:mb-4 p-3 sm:p-4 bg-slate-50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded-lg">
          <div class="flex flex-col sm:flex-row items-start sm:items-center gap-3 sm:gap-4">
            <div class="flex-1 w-full sm:w-auto">
              <label class="block text-xs text-slate-600 dark:text-slate-400 mb-1">
                {{ $t('envConfig.nginx.version') }}
              </label>
              <CustomSelect 
                v-model="nginx.version" 
                :options="nginxVersionOptions"
                :placeholder="$t('envConfig.nginx.versionPlaceholder')" 
              />
            </div>
            <div class="w-full sm:w-32" v-if="nginxVersions.find(v => v.id === nginx.version)?.show_port !== false">
              <label class="block text-xs text-slate-600 dark:text-slate-400 mb-1">{{ $t('envConfig.hostPort') }}</label>
              <input v-model.number="nginx.host_port" type="number" class="w-full bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-900 dark:text-slate-200 outline-none focus:ring-2 focus:ring-blue-500" />
            </div>
            <button @click="removeNginxVersion(idx)" class="w-full sm:w-auto mt-2 sm:mt-5 text-rose-400 hover:text-rose-300 text-sm">{{ $t('envConfig.remove') }}</button>
          </div>
        </div>
        <div v-if="nginxServices.length === 0" class="text-center py-8 text-slate-500 dark:text-slate-500 text-sm">
          点击上方“{{ $t('envConfig.addVersion') }}”按钮添加 Nginx 服务
        </div>
      </section>

      <!-- General Settings -->
      <section class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-4 sm:p-6">
        <h2 class="text-lg font-bold mb-4 text-slate-900 dark:text-slate-200">{{ $t('envConfig.general.title') }}</h2>
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          <div>
            <label class="block text-xs text-slate-600 dark:text-slate-400 mb-1">{{ $t('envConfig.general.workspace') }}</label>
            <input 
              :value="workspacePath" 
              readonly
              class="w-full bg-slate-100 dark:bg-slate-800/50 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-700 dark:text-slate-300 cursor-not-allowed"
            />
          </div>
          <div>
            <label class="block text-xs text-slate-600 dark:text-slate-400 mb-1">{{ $t('envConfig.general.sourceDir') }}</label>
            <input v-model="sourceDir" type="text" placeholder="./www" class="w-full bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-900 dark:text-slate-200 outline-none focus:ring-2 focus:ring-blue-500" />
          </div>
          <div>
            <label class="block text-xs text-slate-600 dark:text-slate-400 mb-1">{{ $t('envConfig.general.timezone') }}</label>
            <CustomSelect 
              :modelValue="showCustomTimezoneInput ? '__custom__' : timezone"
              :options="timezoneOptions"
              :placeholder="$t('envConfig.general.timezonePlaceholder')" 
              @change="handleTimezoneChange"
            />
            <div v-if="showCustomTimezoneInput" class="mt-2">
              <input 
                v-model="customTimezone"
                @input="handleCustomTimezoneChange"
                type="text"
                placeholder="例如：Europe/Moscow"
                class="w-full bg-white dark:bg-slate-800 border border-blue-500/50 rounded-lg px-3 py-2 text-sm text-slate-900 dark:text-slate-200 outline-none focus:ring-2 focus:ring-blue-500"
              />
              <p class="text-xs text-slate-500 dark:text-slate-500 mt-1">
                💡 提示：请输入有效的 IANA 时区标识符，如 "Europe/Moscow"、"Pacific/Auckland"
              </p>
            </div>
          </div>
        </div>
      </section>
    </div>

    <!-- Preview Modal -->
    <div v-if="showPreviewModal" class="fixed inset-0 bg-black/70 backdrop-blur-sm flex items-center justify-center z-50 p-3 sm:p-4" @click.self="showPreviewModal = false">
      <div class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-700 rounded-xl w-full max-w-4xl sm:max-w-6xl mx-auto max-h-[90vh] flex flex-col shadow-2xl">
        <div class="flex justify-between items-center p-4 sm:p-6 border-b border-slate-200 dark:border-slate-700">
          <h2 class="text-lg sm:text-xl font-bold text-slate-900 dark:text-slate-200">{{ $t('envConfig.previewModal.title') }}</h2>
          <button @click="showPreviewModal = false" class="text-slate-500 dark:text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 text-2xl">&times;</button>
        </div>
        <div class="flex-1 overflow-y-auto p-4 sm:p-6">
          <div class="grid grid-cols-1 lg:grid-cols-2 gap-3 sm:gap-4">
            <div>
              <div class="text-xs text-slate-500 dark:text-slate-400 mb-2 uppercase tracking-wider">.env</div>
              <pre class="bg-slate-100 dark:bg-black/40 p-3 sm:p-4 rounded-lg text-xs text-green-600 dark:text-green-300/80 border border-slate-200 dark:border-slate-700 max-h-80 sm:max-h-96 overflow-y-auto font-mono whitespace-pre-wrap">{{ previewEnv }}</pre>
            </div>
            <div>
              <div class="text-xs text-slate-500 dark:text-slate-400 mb-2 uppercase tracking-wider">docker-compose.yml</div>
              <pre class="bg-slate-100 dark:bg-black/40 p-3 sm:p-4 rounded-lg text-xs text-blue-600 dark:text-blue-300/80 border border-slate-200 dark:border-slate-700 max-h-80 sm:max-h-96 overflow-y-auto font-mono whitespace-pre-wrap">{{ previewCompose }}</pre>
            </div>
          </div>
        </div>
        <div class="p-4 sm:p-6 border-t border-slate-200 dark:border-slate-700 flex flex-col sm:flex-row justify-end gap-3">
          <button @click="showPreviewModal = false" class="w-full sm:w-auto px-5 py-2 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 border border-slate-300 dark:border-slate-700 text-slate-700 dark:text-slate-300 rounded-lg font-medium transition">
            {{ $t('envConfig.previewModal.close') }}
          </button>
          <button @click="handleApply" :disabled="applying" class="w-full sm:w-auto px-5 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg font-medium transition disabled:opacity-50">
            {{ applying ? $t('envConfig.applying') : $t('envConfig.previewModal.applyConfig') }}
          </button>
        </div>
      </div>
    </div>

    <!-- Start Environment Confirmation Dialog -->
    <div v-if="showStartConfirm" class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50">
      <div class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-700 rounded-xl p-8 max-w-md w-full shadow-2xl">
        <h2 class="text-2xl font-bold text-slate-900 dark:text-slate-200 mb-4">{{ $t('dashboard.startConfirm.title') }}</h2>
        <p class="text-slate-600 dark:text-slate-400 mb-6">
          {{ $t('dashboard.startConfirm.message', { proxy: '' }) }}
          <strong>{{ $t('dashboard.startConfirm.proxy') }}</strong>
        </p>

        <div class="space-y-4">
          <div class="flex gap-3">
            <button 
              @click="showStartConfirm = false"
              class="flex-1 px-4 py-2 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-300 rounded-lg font-medium transition"
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
