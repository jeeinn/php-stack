<template>
  <div class="step3-deploy">
    <h3>步骤 3/3: 启动开发环境</h3>

    <!-- 部署状态 -->
    <div class="deploy-status" :class="status">
      <div v-if="status === 'deploying'" class="status-indicator">
        <div class="spinner"></div>
        <p>📦 正在部署环境...</p>
      </div>
      <div v-else-if="status === 'success'" class="status-indicator">
        <span class="success-icon">✅</span>
        <p>环境已就绪！</p>
      </div>
      <div v-else-if="status === 'failed'" class="status-indicator">
        <span class="error-icon">❌</span>
        <p>部署失败</p>
      </div>
    </div>

    <!-- 实时日志 -->
    <div class="logs-section">
      <h4>📋 实时日志:</h4>
      <div class="logs-container">
        <div 
          v-for="(log, index) in logs" 
          :key="index"
          class="log-line"
          :class="getLogClass(log)"
        >
          {{ log }}
        </div>
      </div>
    </div>

    <!-- 连接信息（成功后显示） -->
    <div v-if="status === 'success'" class="connection-info">
      <h4>🔗 连接信息:</h4>
      <div class="info-grid">
        <div class="info-item">
          <label>MySQL:</label>
          <code>localhost:3306</code>
        </div>
        <div class="info-item">
          <label>Root 密码:</label>
          <code>root123</code>
        </div>
        <div class="info-item">
          <label>Redis:</label>
          <code>localhost:6379</code>
        </div>
        <div class="info-item">
          <label>Nginx:</label>
          <code>http://localhost</code>
        </div>
      </div>
    </div>

    <!-- 操作按钮 -->
    <div class="actions">
      <button 
        v-if="status !== 'deploying'" 
        @click="$emit('prev')" 
        class="btn-prev"
      >
        ← 上一步
      </button>
      <button 
        v-if="status === 'success' || status === 'failed'" 
        @click="$emit('reset')" 
        class="btn-reset"
      >
        🔄 重新配置
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{
  spec: any;
  deployStatus: 'idle' | 'deploying' | 'success' | 'failed';
  logs: string[];
}>();

const emit = defineEmits(['prev', 'reset']);

// 获取状态类名
const status = computed(() => props.deployStatus);

// 根据日志内容返回样式类
function getLogClass(log: string) {
  if (log.includes('✅')) return 'log-success';
  if (log.includes('❌')) return 'log-error';
  if (log.includes('⚠️')) return 'log-warning';
  if (log.includes('🔨') || log.includes('📦')) return 'log-info';
  return '';
}
</script>

<style scoped>
.step3-deploy h3 {
  font-size: 1.5rem;
  margin-bottom: 1.5rem;
  color: #1a1a1a;
}

/* 部署状态 */
.deploy-status {
  padding: 1.5rem;
  border-radius: 8px;
  margin-bottom: 1.5rem;
  text-align: center;
}

.deploy-status.deploying {
  background: #dbeafe;
  border: 1px solid #93c5fd;
}

.deploy-status.success {
  background: #d1fae5;
  border: 1px solid #6ee7b7;
}

.deploy-status.failed {
  background: #fee2e2;
  border: 1px solid #fca5a5;
}

.status-indicator {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 1rem;
  font-size: 1.1rem;
  font-weight: 600;
}

.spinner {
  width: 24px;
  height: 24px;
  border: 3px solid #3b82f6;
  border-top-color: transparent;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.success-icon, .error-icon {
  font-size: 1.5rem;
}

/* 日志区域 */
.logs-section h4 {
  margin-bottom: 1rem;
  color: #374151;
}

.logs-container {
  background: #1e1e1e;
  border-radius: 8px;
  padding: 1rem;
  max-height: 300px;
  overflow-y: auto;
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 0.875rem;
  line-height: 1.6;
}

.log-line {
  color: #d4d4d4;
  padding: 0.25rem 0;
}

.log-success {
  color: #6ee7b7;
}

.log-error {
  color: #fca5a5;
}

.log-warning {
  color: #fcd34d;
}

.log-info {
  color: #93c5fd;
}

/* 连接信息 */
.connection-info {
  background: #f0fdf4;
  border: 1px solid #86efac;
  border-radius: 8px;
  padding: 1rem;
  margin: 1.5rem 0;
}

.connection-info h4 {
  margin-bottom: 1rem;
  color: #166534;
}

.info-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 1rem;
}

.info-item {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.info-item label {
  font-size: 0.75rem;
  color: #6b7280;
}

.info-item code {
  background: white;
  padding: 0.5rem;
  border-radius: 4px;
  font-size: 0.875rem;
  color: #166534;
  border: 1px solid #86efac;
}

/* 按钮 */
.actions {
  display: flex;
  justify-content: space-between;
  gap: 1rem;
  margin-top: 2rem;
}

.btn-prev, .btn-reset {
  padding: 0.75rem 1.5rem;
  border: none;
  border-radius: 8px;
  font-size: 1rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-prev {
  background: #f3f4f6;
  color: #374151;
}

.btn-prev:hover {
  background: #e5e7eb;
}

.btn-reset {
  background: #3b82f6;
  color: white;
}

.btn-reset:hover {
  background: #2563eb;
}
</style>
