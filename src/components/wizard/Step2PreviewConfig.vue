<template>
  <div class="step2-preview">
    <h3>步骤 2/3: 预览 docker-compose 配置</h3>

    <!-- YAML 预览 -->
    <div class="yaml-preview">
      <pre><code>{{ yamlContent || '加载中...' }}</code></pre>
    </div>

    <!-- 配置摘要 -->
    <div class="config-summary" v-if="summary">
      <p>⚠️ 将创建 {{ summary.serviceCount }} 个容器，占用端口: {{ summary.ports.join(', ') }}</p>
      <p>💾 数据目录: ./data</p>
    </div>

    <!-- 操作按钮 -->
    <div class="actions">
      <button @click="$emit('prev')" class="btn-prev">← 上一步</button>
      <button @click="$emit('next')" class="btn-next" :disabled="!yamlContent">
        🚀 一键启动
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';

const props = defineProps<{
  spec: any;
}>();

const emit = defineEmits(['prev', 'next']);

const yamlContent = ref('');
const summary = ref<any>(null);

// 加载 YAML 预览
onMounted(async () => {
  try {
    const yaml = await invoke<string>('generate_compose_preview', {
      spec: props.spec,
    });
    yamlContent.value = yaml;
    
    // 解析摘要信息
    parseSummary(props.spec);
  } catch (error: any) {
    yamlContent.value = `❌ 生成配置失败: ${error}`;
  }
});

// 解析配置摘要
function parseSummary(spec: any) {
  const serviceCount = spec.services.length;
  const ports: string[] = [];
  
  spec.services.forEach((service: any) => {
    if (service.ports) {
      Object.keys(service.ports).forEach(port => {
        ports.push(port);
      });
    }
  });
  
  summary.value = {
    serviceCount,
    ports,
  };
}
</script>

<style scoped>
.step2-preview h3 {
  font-size: 1.5rem;
  margin-bottom: 1.5rem;
  color: #f1f5f9; /* text-slate-100 */
}

.yaml-preview {
  background: #0f172a; /* bg-slate-900 */
  border: 1px solid #1e293b; /* border-slate-800 */
  border-radius: 8px;
  padding: 1.5rem;
  overflow-x: auto;
  max-height: 400px;
  overflow-y: auto;
  margin-bottom: 1.5rem;
}

.yaml-preview pre {
  margin: 0;
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 0.875rem;
  line-height: 1.6;
  color: #e2e8f0; /* text-slate-200 */
}

.config-summary {
  background: #451a03; /* bg-amber-950 */
  border: 1px solid #92400e; /* border-amber-700 */
  border-radius: 8px;
  padding: 1rem;
  margin-bottom: 1.5rem;
}

.config-summary p {
  margin: 0.5rem 0;
  font-size: 0.875rem;
  color: #fcd34d; /* text-amber-300 */
}

.actions {
  display: flex;
  justify-content: space-between;
  gap: 1rem;
}

.btn-prev, .btn-next {
  padding: 0.75rem 1.5rem;
  border: none;
  border-radius: 8px;
  font-size: 1rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-prev {
  background: #1e293b; /* bg-slate-800 */
  color: #cbd5e1; /* text-slate-300 */
  border: 1px solid #334155; /* border-slate-700 */
}

.btn-prev:hover {
  background: #334155; /* bg-slate-700 */
}

.btn-next {
  background: #10b981; /* bg-emerald-500 */
  color: white;
}

.btn-next:hover:not(:disabled) {
  background: #059669; /* bg-emerald-600 */
}

.btn-next:disabled {
  background: #334155; /* bg-slate-700 */
  color: #64748b; /* text-slate-500 */
  cursor: not-allowed;
}
</style>
