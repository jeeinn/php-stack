<template>
  <div class="step1-select-stack">
    <h3>步骤 1/3: 选择技术栈组件</h3>

    <!-- 必选服务 -->
    <div class="service-section">
      <h4>☑ 必选服务</h4>
      
      <!-- PHP -->
      <div class="service-item">
        <div class="service-header">
          <label class="checkbox-label">
            <input 
              type="checkbox" 
              v-model="services.php.enabled"
              disabled
            />
            <span class="service-name">PHP</span>
          </label>
          <select v-model="services.php.version" class="version-select">
            <option value="8.2">8.2 ⭐</option>
            <option value="8.1">8.1</option>
            <option value="8.0">8.0</option>
            <option value="7.4">7.4</option>
          </select>
        </div>
        
        <!-- PHP 扩展 -->
        <div class="extensions-grid" v-if="services.php.enabled">
          <label v-for="ext in phpExtensions" :key="ext.name" class="extension-checkbox">
            <input 
              type="checkbox" 
              v-model="ext.selected"
            />
            <span>{{ ext.label }}</span>
          </label>
        </div>
      </div>

      <!-- MySQL -->
      <div class="service-item">
        <div class="service-header">
          <label class="checkbox-label">
            <input 
              type="checkbox" 
              v-model="services.mysql.enabled"
              disabled
            />
            <span class="service-name">MySQL</span>
          </label>
          <select v-model="services.mysql.version" class="version-select">
            <option value="8.0">8.0 ⭐</option>
            <option value="5.7">5.7</option>
          </select>
        </div>
        <p class="service-hint">Root 密码: root123（固定）</p>
      </div>

      <!-- Redis -->
      <div class="service-item">
        <div class="service-header">
          <label class="checkbox-label">
            <input 
              type="checkbox" 
              v-model="services.redis.enabled"
              disabled
            />
            <span class="service-name">Redis</span>
          </label>
          <select v-model="services.redis.version" class="version-select">
            <option value="7.0">7.0 ⭐</option>
            <option value="6.2">6.2</option>
          </select>
        </div>
      </div>
    </div>

    <!-- 可选服务 -->
    <div class="service-section">
      <h4>☑ 可选服务</h4>
      
      <!-- Nginx -->
      <div class="service-item">
        <div class="service-header">
          <label class="checkbox-label">
            <input 
              type="checkbox" 
              v-model="services.nginx.enabled"
            />
            <span class="service-name">Nginx</span>
          </label>
          <select v-model="services.nginx.version" class="version-select" v-if="services.nginx.enabled">
            <option value="1.24">1.24 ⭐</option>
            <option value="1.22">1.22</option>
          </select>
        </div>
        <p class="service-hint" v-if="services.nginx.enabled">站点: localhost  根目录: /var/www/html</p>
      </div>
    </div>

    <!-- 镜像源配置 -->
    <div class="mirror-config-section">
      <h4>⚙️ 镜像源配置（加速下载）</h4>
      <div class="mirror-grid">
        <div class="mirror-item">
          <label>APT:</label>
          <select v-model="mirrorConfig.apt_mirror">
            <option value="default">默认</option>
            <option value="aliyun">阿里云</option>
            <option value="tsinghua">清华大学</option>
            <option value="ustc">中科大</option>
          </select>
        </div>
        <div class="mirror-item">
          <label>Composer:</label>
          <select v-model="mirrorConfig.composer_mirror">
            <option value="default">默认</option>
            <option value="aliyun">阿里云</option>
            <option value="huaweicloud">华为云</option>
          </select>
        </div>
      </div>
      <div class="mirror-actions">
        <button @click="testMirrorConnection" class="btn-secondary">测试连接</button>
        <button @click="saveMirrorConfig" class="btn-primary">保存配置</button>
      </div>
      <p v-if="testResult" class="test-result">{{ testResult }}</p>
    </div>

    <!-- 操作按钮 -->
    <div class="actions">
      <button @click="$emit('next')" class="btn-next">下一步 →</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';

const props = defineProps<{
  spec: any;
}>();

const emit = defineEmits(['update:spec', 'next']);

// 服务配置
const services = reactive({
  php: { enabled: true, version: '8.2' },
  mysql: { enabled: true, version: '8.0' },
  redis: { enabled: true, version: '7.0' },
  nginx: { enabled: true, version: '1.24' },
});

// PHP 扩展
const phpExtensions = ref([
  { name: 'mysqli', label: 'mysqli', selected: true },
  { name: 'pdo_mysql', label: 'pdo_mysql', selected: true },
  { name: 'redis', label: 'redis', selected: true },
  { name: 'gd', label: 'gd', selected: true },
  { name: 'mbstring', label: 'mbstring', selected: true },
  { name: 'curl', label: 'curl', selected: false },
  { name: 'zip', label: 'zip', selected: false },
]);

// 镜像源配置
const mirrorConfig = reactive({
  apt_mirror: 'aliyun',
  composer_mirror: 'aliyun',
});

const testResult = ref('');

// 监听变化，更新 spec
watch([services, phpExtensions], () => {
  updateSpec();
}, { deep: true });

// 更新环境规格
function updateSpec() {
  const selectedServices = [];

  // PHP
  if (services.php.enabled) {
    const extensions = phpExtensions.value
      .filter(ext => ext.selected)
      .map(ext => ext.name);
    
    selectedServices.push({
      software_type: 'PHP',
      version: services.php.version,
      ports: { '9000': 9000 },
      extensions: extensions.length > 0 ? extensions : null,
    });
  }

  // MySQL
  if (services.mysql.enabled) {
    selectedServices.push({
      software_type: 'MySQL',
      version: services.mysql.version,
      ports: { '3306': 3306 },
      extensions: null,
    });
  }

  // Redis
  if (services.redis.enabled) {
    selectedServices.push({
      software_type: 'Redis',
      version: services.redis.version,
      ports: { '6379': 6379 },
      extensions: null,
    });
  }

  // Nginx
  if (services.nginx.enabled) {
    selectedServices.push({
      software_type: 'Nginx',
      version: services.nginx.version,
      ports: { '80': 80 },
      extensions: null,
    });
  }

  emit('update:spec', {
    services: selectedServices,
    network_name: 'php-stack-network',
  });
}

// 测试镜像源连接
async function testMirrorConnection() {
  try {
    testResult.value = '测试中...';
    const result = await invoke<boolean>('test_mirror_connection', {
      source: mirrorConfig.apt_mirror,
    });
    testResult.value = result ? '✅ 连接成功' : '❌ 连接失败';
  } catch (error: any) {
    testResult.value = `❌ 测试失败: ${error}`;
  }
}

// 保存镜像源配置
async function saveMirrorConfig() {
  try {
    await invoke('update_mirror_config', {
      config: {
        apt_mirror: mirrorConfig.apt_mirror,
        composer_mirror: mirrorConfig.composer_mirror,
        pypi_mirror: 'aliyun',
        npm_mirror: 'taobao',
      },
    });
    alert('✅ 配置已保存');
  } catch (error: any) {
    alert(`❌ 保存失败: ${error}`);
  }
}

// 初始化
updateSpec();
</script>

<style scoped>
.step1-select-stack h3 {
  font-size: 1.5rem;
  margin-bottom: 1.5rem;
  color: #f1f5f9; /* text-slate-100 */
}

.service-section {
  margin-bottom: 2rem;
}

.service-section h4 {
  font-size: 1.1rem;
  margin-bottom: 1rem;
  color: #cbd5e1; /* text-slate-300 */
}

.service-item {
  background: #1e293b; /* bg-slate-800 */
  border: 1px solid #334155; /* border-slate-700 */
  border-radius: 8px;
  padding: 1rem;
  margin-bottom: 1rem;
}

.service-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 0.5rem;
}

.checkbox-label {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  cursor: pointer;
}

.service-name {
  font-weight: 600;
  color: #f1f5f9; /* text-slate-100 */
}

.version-select {
  padding: 0.25rem 0.5rem;
  background: #0f172a; /* bg-slate-900 */
  border: 1px solid #475569; /* border-slate-600 */
  border-radius: 4px;
  font-size: 0.875rem;
  color: #e2e8f0; /* text-slate-200 */
}

.service-hint {
  font-size: 0.875rem;
  color: #94a3b8; /* text-slate-400 */
  margin-top: 0.5rem;
}

/* PHP 扩展网格 */
.extensions-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
  gap: 0.5rem;
  margin-top: 1rem;
}

.extension-checkbox {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  font-size: 0.875rem;
  cursor: pointer;
  color: #cbd5e1; /* text-slate-300 */
}

.extension-checkbox input[type="checkbox"] {
  accent-color: #3b82f6; /* blue-500 */
}

/* 镜像源配置 */
.mirror-config-section {
  background: #1e3a8a; /* bg-blue-900 */
  border: 1px solid #1e40af; /* border-blue-800 */
  border-radius: 8px;
  padding: 1rem;
  margin-bottom: 2rem;
}

.mirror-config-section h4 {
  margin-bottom: 1rem;
  color: #93c5fd; /* text-blue-300 */
}

.mirror-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 1rem;
  margin-bottom: 1rem;
}

.mirror-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.mirror-item label {
  font-size: 0.875rem;
  color: #cbd5e1; /* text-slate-300 */
  min-width: 80px;
}

.mirror-item select {
  flex: 1;
  padding: 0.25rem 0.5rem;
  background: #0f172a; /* bg-slate-900 */
  border: 1px solid #475569; /* border-slate-600 */
  border-radius: 4px;
  color: #e2e8f0; /* text-slate-200 */
}

.mirror-actions {
  display: flex;
  gap: 0.5rem;
}

.test-result {
  margin-top: 0.5rem;
  font-size: 0.875rem;
  color: #6ee7b7; /* text-emerald-300 */
}

/* 按钮 */
.actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 2rem;
}

.btn-next {
  padding: 0.75rem 2rem;
  background: #3b82f6; /* bg-blue-500 */
  color: white;
  border: none;
  border-radius: 8px;
  font-size: 1rem;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.2s;
}

.btn-next:hover {
  background: #2563eb; /* bg-blue-600 */
}

.btn-primary {
  padding: 0.5rem 1rem;
  background: #3b82f6; /* bg-blue-500 */
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}

.btn-primary:hover {
  background: #2563eb; /* bg-blue-600 */
}

.btn-secondary {
  padding: 0.5rem 1rem;
  background: #1e293b; /* bg-slate-800 */
  color: #60a5fa; /* text-blue-400 */
  border: 1px solid #3b82f6; /* border-blue-500 */
  border-radius: 4px;
  cursor: pointer;
}

.btn-secondary:hover {
  background: #334155; /* bg-slate-700 */
}
</style>
