<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useI18n } from 'vue-i18n';
import {
  getWorkspaceInfo,
  checkConfigFilesExist,
  getVersionMappings,
  loadExistingConfig as apiLoadExistingConfig,
  generateEnvConfig,
  previewCompose,
  applyEnvConfig,
  checkServiceImagesPresence,
  pullServiceImages,
  openServiceConfig,
  startEnvironment,
  normalizeError,
} from '../api';
import { open } from '@tauri-apps/plugin-shell';
import type { ServiceEntry, EnvConfig, VersionInfo, ImagePresence, PullImageResultItem } from '../types/env-config';
import { showToast } from '../composables/useToast';
import { showConfirm } from '../composables/useConfirmDialog';
import CustomSelect from './CustomSelect.vue';
import VersionHelpModal from './VersionHelpModal.vue';
import ImagePullConfirmModal from './ImagePullConfirmModal.vue';
import { WORKSPACE_CHANGED_EVENT } from '../utils/workspaceEvents';

const { t } = useI18n();

const emit = defineEmits<{
  (e: 'request-switch-tab', tabName: string): void;
}>();

// Available versions (将从后端动态加载)
const phpVersions = ref<VersionInfo[]>([]);
const mysqlVersions = ref<VersionInfo[]>([]);
const redisVersions = ref<VersionInfo[]>([]);
const nginxVersions = ref<VersionInfo[]>([]);

// 版本清单加载状态：失败时不回退到内置硬编码列表，而是展示可操作的错误态，
// 避免用户自己在 version_manifest.json 里加的版本"看起来没生效"。
const versionLoadError = ref(false);
const versionLoadRetrying = ref(false);

// PHP 扩展预设列表（扁平化）
const commonExtensions = [
  'pdo_mysql', 'mysqli', 'mbstring', 'gd', 'curl', 'opcache', 'zip', 'bcmath', 'intl',
  'pdo_pgsql', 'pdo_sqlite', 'mongodb', 'redis', 'memcached', 'amqp',
  'swoole', 'openswoole', 'parallel',
  'xdebug', 'blackfire', 'tidy', 'soap',
  'imagick', 'exif', 'pcntl', 'sockets'
];

const customExtInput = ref<Record<number, string>>({}); // 为每个 PHP 服务维护独立的自定义扩展输入
// 为每个 PHP 服务维护独立的扩展面板展开状态
const phpExtensionsPanelState = ref<Record<number, boolean>>({});

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

// ==================== Phase 3: 版本帮助 / 镜像探测 / 拉取确认 ====================

/// 版本帮助弹窗（4 个版本下拉框旁的 ? 按钮触发）
const showVersionHelp = ref(false);

/// 镜像本地存在性探测结果（弹窗前的数据缓存）
const imagePresences = ref<ImagePresence[]>([]);
const showPullConfirm = ref(false);
const pulling = ref(false);
/// 单镜像拉取进度（tag -> 0-100）：开始拉=10，成功=100，失败=0
const pullProgress = ref<Record<string, number>>({});
/// 拉取进行中的状态文案，如「正在拉取 php:8.2-fpm（1/3）」
const pullStatusText = ref('');
/// 用户是否在「覆盖确认」弹窗里勾选了备份。
///
/// 必须是 ref 而非 handleApply 的局部变量：走「镜像缺失 → 拉取 → 再 apply」
/// 这条路径时，handleApply 已经返回，局部变量会丢失，导致备份选择被静默丢弃。
const enableBackup = ref(false);

/// 打开版本帮助弹窗（"?" 按钮）
function openVersionHelp() {
  showVersionHelp.value = true;
}

// Load existing config on mount
onMounted(async () => {
  await loadWorkspaceInfo();
  await loadVersionMappings();
  await checkEnvFileExists();
  await loadExistingConfig();
  window.addEventListener(WORKSPACE_CHANGED_EVENT, loadWorkspaceInfo);
});

onUnmounted(() => {
  window.removeEventListener(WORKSPACE_CHANGED_EVENT, loadWorkspaceInfo);
});

/// 工作区路径展示（回退告警已提升为 App 全局横幅，此处只显示配置值）
async function loadWorkspaceInfo() {
  try {
    const info = await getWorkspaceInfo();
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
    const existingFiles = await checkConfigFilesExist();
    hasEnvFile.value = existingFiles.some(f => f.includes('.env'));
  } catch (e) {
    console.error('[EnvConfig] failed to check config file:', e);
    hasEnvFile.value = false;
  }
}

// 从后端加载版本映射
async function loadVersionMappings() {
  try {
    const mappings = await getVersionMappings();
    
    // 提取版本信息列表（包含 id、display_name、image_tag、service_dir 等完整信息）
    if (mappings.php) {
      phpVersions.value = mappings.php;
    }
    if (mappings.mysql) {
      mysqlVersions.value = mappings.mysql;
    }
    if (mappings.redis) {
      redisVersions.value = mappings.redis;
    }
    if (mappings.nginx) {
      nginxVersions.value = mappings.nginx;
    }

    // 加载成功：清除上一次可能残留的错误态（用户修正清单后重试成功时）
    versionLoadError.value = false;
  } catch (e) {
    console.error('[EnvConfig] failed to load version mapping:', e);
    // 不再回退到内置的硬编码列表：那份列表会与 services/version_manifest.json 脱节，
    // 导致用户自行添加的版本"看起来没生效"。改为展示可操作的错误态，
    // 引导用户检查清单文件后重试。
    versionLoadError.value = true;
  } finally {
    versionLoadRetrying.value = false;
  }
}

// 重试加载版本映射（用户修正 services/version_manifest.json 后点击）
async function retryLoadVersionMappings() {
  if (versionLoadRetrying.value) return;
  versionLoadRetrying.value = true;
  await loadVersionMappings();
}

// 错误信息格式化
// 后端错误串已统一为英文，这里按词边界匹配分类：
// 子串匹配会让 already 命中 read、HOST_PORT 命中 port，产生误分类。
const hasWord = (text: string, word: string) =>
  new RegExp(`\\b${word}\\b`, 'i').test(text);

function formatErrorMessage(error: unknown): string {
  const errorMsg = normalizeError(error);
  
  if (errorMsg.includes('Docker') || errorMsg.includes('docker')) {
    if (errorMsg.includes('not running') || errorMsg.includes('unavailable')) {
      return t('envConfig.error.dockerNotRunning');
    }
    if (errorMsg.includes('permission')) {
      return t('envConfig.error.permissionDenied');
    }
  }
  
  if (hasWord(errorMsg, 'port')) {
    return t('envConfig.error.portConflict', { error: errorMsg });
  }
  
  if (hasWord(errorMsg, 'read')) {
    return t('envConfig.error.readFailed');
  }
  if (hasWord(errorMsg, 'write')) {
    return t('envConfig.error.writeFailed');
  }
  
  if (hasWord(errorMsg, 'parse')) {
    return t('envConfig.error.parseFailed');
  }
  
  return t('envConfig.error.default', { error: errorMsg });
}

// 显示错误消息
function showError(message: string) {
  showToast(message, 'error', 5000); // 错误消息显示时间更长
}

async function loadExistingConfig() {
  try {
    const config = await apiLoadExistingConfig();
    
    if (config) {
      // Parse services
      const phpSvcs: ServiceEntry[] = [];
      const mysqlSvcs: ServiceEntry[] = [];
      const redisSvcs: ServiceEntry[] = [];
      const nginxSvcs: ServiceEntry[] = [];
      
      config.services.forEach(s => {
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
      
      
      phpServices.value = phpSvcs.length > 0 ? phpSvcs : [{
        service_type: 'PHP',
        version: 'php82',
        host_port: 9000,
        extensions: ['pdo_mysql', 'mysqli', 'mbstring', 'gd', 'curl', 'opcache'],
      }];
      
      // 初始化 PHP 服务的扩展面板状态（全部关闭）
      phpExtensionsPanelState.value = {};
      phpServices.value.forEach((_, idx) => {
        phpExtensionsPanelState.value[idx] = false;
      });
      
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
      
    } else {
      // Default config
      phpServices.value = [{
        service_type: 'PHP',
        version: 'php82',
        host_port: 9000,
        extensions: ['pdo_mysql', 'mysqli', 'mbstring', 'gd', 'curl', 'opcache'],
      }];
      // 初始化默认 PHP 服务的扩展面板状态
      phpExtensionsPanelState.value = { 0: false };
      mysqlServices.value = [{
        service_type: 'MySQL',
        version: 'mysql80',
        host_port: 3306,
      }];
    }
  } catch (e) {
    console.error('[EnvConfig] failed to load config:', e);
    // Use defaults
    phpServices.value = [{
      service_type: 'PHP',
      version: 'php82',
      host_port: 9000,
      extensions: ['pdo_mysql', 'mysqli', 'mbstring', 'gd', 'curl', 'opcache'],
    }];
    // 初始化默认 PHP 服务的扩展面板状态
    phpExtensionsPanelState.value = { 0: false };
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
  const newIndex = phpServices.value.length;
  phpServices.value.push({
    service_type: 'PHP',
    version: available[0].id,
    host_port: 9000 + phpServices.value.length,
    extensions: ['pdo_mysql', 'mysqli', 'mbstring', 'curl'],
  });
  // 初始化新添加的 PHP 服务的扩展面板状态与自定义扩展输入
  phpExtensionsPanelState.value[newIndex] = false;
  customExtInput.value[newIndex] = '';
}

function removePhpVersion(index: number) {
  if (phpServices.value.length <= 1) return;
  phpServices.value.splice(index, 1);
  // 清理已删除服务的状态
  delete phpExtensionsPanelState.value[index];
  delete customExtInput.value[index];
  // 重新索引后续服务的状态
  const newState: Record<number, boolean> = {};
  Object.keys(phpExtensionsPanelState.value).forEach(key => {
    const numKey = Number(key);
    if (numKey > index) {
      newState[numKey - 1] = phpExtensionsPanelState.value[numKey];
    } else {
      newState[numKey] = phpExtensionsPanelState.value[numKey];
    }
  });
  phpExtensionsPanelState.value = newState;
  const newInputs: Record<number, string> = {};
  Object.keys(customExtInput.value).forEach(key => {
    const numKey = Number(key);
    if (numKey > index) {
      newInputs[numKey - 1] = customExtInput.value[numKey];
    } else {
      newInputs[numKey] = customExtInput.value[numKey];
    }
  });
  customExtInput.value = newInputs;
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

  // 解析该服务自己的自定义扩展输入
  const customExts = (customExtInput.value[phpIndex] || '')
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
      generateEnvConfig(config),
      previewCompose(config),
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
  
  // 检查配置文件是否存在（结果写入 enableBackup ref，供后续拉取路径复用）
  enableBackup.value = false;
  try {
    const existingFiles = await checkConfigFilesExist();
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
        enableBackup.value = result.checkboxValue;
      }
    }
  } catch (e) {
    console.error('failed to check config file:', e);
    // 如果检查失败，继续执行（不阻断用户操作）
  }

  // ========== Phase 3 前置：探测镜像本地存在性 ==========
  applying.value = true;
  try {
    const config = buildConfig();
    const presences = await checkServiceImagesPresence(config);
    imagePresences.value = presences;
    const missing = presences.filter(p => p.status === 'missing');

    if (missing.length === 0) {
      // 全部已存在 → 跳过拉取，直接 apply
      await doApplyCore(config, enableBackup.value);
    } else {
      // 有缺失 → 弹"待拉取"确认
      showPullConfirm.value = true;
      // 不在这里关闭 applying：pullConfirm 弹窗期间由前端维持视觉态
      // （弹窗关闭回调里再处理下一步）
    }
  } catch (e) {
    console.error('[EnvConfig] failed to check image existence:', e);
    showError(formatErrorMessage(e));
    applying.value = false;
  }
}

/// 用户在 pullConfirm 弹窗点击"确认拉取"：逐个拉取缺失镜像，然后继续 apply
async function confirmPullAndApply(missingTags: string[]) {
  pulling.value = true;
  pullStatusText.value = '';
  const results: PullImageResultItem[] = [];
  for (const t of missingTags) pullProgress.value[t] = 0;

  try {
    // 逐个拉取，才能在 UI 显示「当前正在拉 xxx / 第 n/m」
    for (let i = 0; i < missingTags.length; i++) {
      const tag = missingTags[i];
      pullStatusText.value = t('envConfig.pullConfirm.pullingItem', {
        tag,
        current: i + 1,
        total: missingTags.length,
      });
      pullProgress.value = { ...pullProgress.value, [tag]: 10 };

      try {
        const batch = await pullServiceImages([tag]);
        const item = batch[0] ?? { tag, success: false, error: 'empty result' };
        results.push(item);
        pullProgress.value = {
          ...pullProgress.value,
          [tag]: item.success ? 100 : 0,
        };
      } catch (e) {
        results.push({ tag, success: false, error: normalizeError(e) });
        pullProgress.value = { ...pullProgress.value, [tag]: 0 };
      }
    }

    pullStatusText.value = '';
    const failures = results.filter(r => !r.success);

    if (failures.length > 0 && failures.length === results.length) {
      // 全部失败：二次确认，避免用户误以为镜像已就绪却静默用模板兜底
      const confirmed = await showConfirm({
        title: t('envConfig.confirmPullFailed.title'),
        message: t('envConfig.confirmPullFailed.message', {
          details: failures.map(f => `• ${f.tag}`).join('\n'),
        }),
        confirmText: t('envConfig.confirmPullFailed.continue'),
        cancelText: t('common.cancel'),
        type: 'warning',
      });
      if (!confirmed) {
        pulling.value = false;
        applying.value = false;
        return;
      }
      showToast(t('envConfig.toast.pullAllFailed'), 'warning', 5000);
    } else if (failures.length > 0) {
      const failSummary = failures.map(f => `${f.tag} (${f.error || 'unknown'})`).join(', ');
      showToast(t('envConfig.toast.pullPartialSuccess', { details: failSummary }), 'warning', 5000);
    } else if (results.length > 0) {
      showToast(t('envConfig.toast.pullAllSuccess', { count: results.length }), 'success', 3000);
    }

    pulling.value = false;
    showPullConfirm.value = false;

    const config = buildConfig();
    imagePresences.value = await checkServiceImagesPresence(config);
    await doApplyCore(config, enableBackup.value);
  } catch (e) {
    pullStatusText.value = '';
    pulling.value = false;
    showPullConfirm.value = false;
    showError(formatErrorMessage(e));
    applying.value = false;
  }
}

/// 用户取消 pullConfirm
function cancelPull() {
  showPullConfirm.value = false;
  applying.value = false;
}

/// 真正应用配置（在镜像确认/拉取完成后调用）
async function doApplyCore(config: EnvConfig, enableBackup: boolean) {
  applying.value = true;
  showNginxHint.value = false;
  try {
    const backedUpFiles = await applyEnvConfig(config, enableBackup);
    
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
    await openServiceConfig(targetDir);
    showToast(t('envConfig.toast.nginxConfigOpened', { dir: targetDir }), 'success');
  } catch (e) {
    console.error('failed to open directory:', e);
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
    const result = await startEnvironment();
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
          class="w-full sm:w-auto px-5 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg font-medium transition disabled:opacity-50 text-white"
        >
          {{ applying ? $t('envConfig.applying') : $t('envConfig.apply') }}
        </button>
        <button
          @click="handleStart"
          :disabled="starting || !hasEnvFile"
          class="w-full sm:w-auto px-5 py-2 bg-green-600 hover:bg-green-700 rounded-lg font-medium transition disabled:opacity-50 disabled:cursor-not-allowed text-white"
          :title="!hasEnvFile ? $t('envConfig.startTooltip') : ''"
        >
          {{ starting ? $t('envConfig.startingEnv') : $t('envConfig.startEnv') }}
        </button>
      </div>
    </header>
    
    <!-- 版本清单加载失败提示：不回退硬编码列表，引导用户修正清单后重试 -->
    <div v-if="versionLoadError" class="mb-4 p-4 sm:p-5 bg-amber-50 dark:bg-amber-950/30 border border-amber-200 dark:border-amber-800 rounded-xl">
      <div class="flex flex-col sm:flex-row items-start sm:items-center gap-3">
        <div class="flex-1">
          <p class="text-sm font-medium text-amber-900 dark:text-amber-200">{{ $t('envConfig.versionList.loadFailed') }}</p>
          <p class="text-xs text-amber-800 dark:text-amber-300 mt-1 leading-relaxed">{{ $t('envConfig.versionList.loadFailedHint') }}</p>
        </div>
        <button
          @click="retryLoadVersionMappings"
          :disabled="versionLoadRetrying"
          class="w-full sm:w-auto shrink-0 px-3 py-1.5 text-sm bg-amber-600 hover:bg-amber-700 text-white rounded-lg transition disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {{ versionLoadRetrying ? $t('envConfig.versionList.retrying') : $t('envConfig.versionList.retry') }}
        </button>
      </div>
    </div>

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
          <div class="flex items-center gap-2">
            <h2 class="text-lg font-bold text-slate-900 dark:text-slate-200">{{ $t('envConfig.php.title') }}</h2>
            <button
              @click="openVersionHelp"
              :title="$t('envConfig.versionHelp.helpButton')"
              class="w-5 h-5 inline-flex items-center justify-center rounded-full bg-slate-200 dark:bg-slate-700 text-slate-500 dark:text-slate-400 hover:bg-blue-100 dark:hover:bg-blue-900 hover:text-blue-600 dark:hover:text-blue-300 text-xs font-bold transition-colors"
              aria-label="help"
            >?</button>
          </div>
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
                @click="phpExtensionsPanelState[idx] = !phpExtensionsPanelState[idx]"
                class="w-full flex justify-between items-center px-3 py-2 bg-slate-50 dark:bg-slate-800/50 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors text-left"
              >
                <span class="text-xs font-medium text-slate-700 dark:text-slate-300">{{ $t('envConfig.php.extensionsPanel') }}</span>
                <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5 text-slate-500 transition-transform duration-200" :class="{ 'rotate-180': phpExtensionsPanelState[idx] }" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7" />
                </svg>
              </button>
              
              <div v-show="phpExtensionsPanelState[idx]" class="p-3 bg-slate-50 dark:bg-slate-900/50 border-t border-slate-200 dark:border-slate-700/50 space-y-3">
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
                    v-model="customExtInput[idx]"
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
                {{ $t('envConfig.general.customTimezoneHint') }}
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
          <button @click="handleApply" :disabled="applying" class="w-full sm:w-auto px-5 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg font-medium transition disabled:opacity-50 text-white">
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

    <!-- Phase 3: 版本帮助弹窗（点击 ? 触发） -->
    <VersionHelpModal
      :open="showVersionHelp"
      @close="showVersionHelp = false"
    />

    <!-- Phase 3: 镜像待拉取确认弹窗（apply 前置） -->
    <ImagePullConfirmModal
      :open="showPullConfirm"
      :presences="imagePresences"
      :pulling="pulling"
      :progress="pullProgress"
      :status-text="pullStatusText"
      @confirm="confirmPullAndApply"
      @cancel="cancelPull"
    />
  </div>
</template>
