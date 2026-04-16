<template>
  <div class="environment-wizard">
    <!-- 标题 -->
    <div class="wizard-header">
      <h2>🚀 快速搭建开发环境</h2>
      <p class="subtitle">3 步完成环境配置，开箱即用</p>
    </div>

    <!-- 步骤指示器 -->
    <div class="step-indicator">
      <div 
        v-for="(step, index) in steps" 
        :key="index"
        class="step-item"
        :class="{ 
          active: currentStep === index + 1,
          completed: currentStep > index + 1 
        }"
      >
        <div class="step-number">{{ index + 1 }}</div>
        <div class="step-title">{{ step.title }}</div>
      </div>
      <div class="step-connector" v-for="i in 2" :key="i"></div>
    </div>

    <!-- 步骤内容 -->
    <div class="step-content">
      <!-- 步骤 1: 选择技术栈 -->
      <Step1SelectStack 
        v-if="currentStep === 1"
        v-model:spec="envSpec"
        @next="currentStep = 2"
      />

      <!-- 步骤 2: 预览配置 -->
      <Step2PreviewConfig
        v-if="currentStep === 2"
        :spec="envSpec"
        @prev="currentStep = 1"
        @next="handleDeploy"
      />

      <!-- 步骤 3: 部署进度 -->
      <Step3DeployEnv
        v-if="currentStep === 3"
        :spec="envSpec"
        :deploy-status="deployStatus"
        :logs="deployLogs"
        @prev="currentStep = 2"
        @reset="resetWizard"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive } from 'vue';
import Step1SelectStack from './wizard/Step1SelectStack.vue';
import Step2PreviewConfig from './wizard/Step2PreviewConfig.vue';
import Step3DeployEnv from './wizard/Step3DeployEnv.vue';
import { invoke } from '@tauri-apps/api/core';

// 步骤定义
const steps = [
  { title: '选择技术栈' },
  { title: '预览配置' },
  { title: '启动环境' },
];

// 当前步骤
const currentStep = ref(1);

// 环境规格
const envSpec = reactive({
  services: [],
  network_name: 'php-stack-network',
});

// 部署状态
const deployStatus = ref<'idle' | 'deploying' | 'success' | 'failed'>('idle');
const deployLogs = ref<string[]>([]);

// 处理部署
async function handleDeploy() {
  currentStep.value = 3;
  deployStatus.value = 'deploying';
  deployLogs.value = ['🚀 开始部署环境...'];

  try {
    // 调用后端部署命令
    const result = await invoke<string>('deploy_environment_with_build', {
      spec: envSpec,
    });

    deployLogs.value.push(`✅ ${result}`);
    deployStatus.value = 'success';
  } catch (error: any) {
    deployLogs.value.push(`❌ 部署失败: ${error}`);
    deployStatus.value = 'failed';
  }
}

// 重置向导
function resetWizard() {
  currentStep.value = 1;
  deployStatus.value = 'idle';
  deployLogs.value = [];
  // 注意：不清空 envSpec，保留用户选择
}
</script>

<style scoped>
.environment-wizard {
  max-width: 900px;
  margin: 0 auto;
  padding: 2rem;
  color: #e2e8f0; /* text-slate-200 */
}

.wizard-header {
  text-align: center;
  margin-bottom: 2rem;
}

.wizard-header h2 {
  font-size: 1.8rem;
  font-weight: bold;
  color: #f1f5f9; /* text-slate-100 */
  margin-bottom: 0.5rem;
}

.subtitle {
  color: #94a3b8; /* text-slate-400 */
  font-size: 1rem;
}

/* 步骤指示器 */
.step-indicator {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  margin-bottom: 2rem;
}

.step-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.5rem;
}

.step-number {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  background: #334155; /* bg-slate-700 */
  color: #94a3b8; /* text-slate-400 */
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: bold;
  transition: all 0.3s;
}

.step-item.active .step-number {
  background: #3b82f6; /* bg-blue-500 */
  color: white;
}

.step-item.completed .step-number {
  background: #10b981; /* bg-emerald-500 */
  color: white;
}

.step-title {
  font-size: 0.875rem;
  color: #94a3b8; /* text-slate-400 */
}

.step-item.active .step-title {
  color: #60a5fa; /* text-blue-400 */
  font-weight: 600;
}

.step-connector {
  width: 60px;
  height: 2px;
  background: #334155; /* bg-slate-700 */
}

/* 步骤内容 - 可滚动 */
.step-content {
  background: #0f172a; /* bg-slate-900 */
  border: 1px solid #1e293b; /* border-slate-800 */
  border-radius: 12px;
  padding: 2rem;
  min-height: 400px;
  max-height: calc(100vh - 280px); /* 限制最大高度 */
  overflow-y: auto; /* 启用垂直滚动 */
}

/* 自定义滚动条样式 */
.step-content::-webkit-scrollbar {
  width: 8px;
}

.step-content::-webkit-scrollbar-track {
  background: #1e293b; /* bg-slate-800 */
  border-radius: 4px;
}

.step-content::-webkit-scrollbar-thumb {
  background: #475569; /* bg-slate-600 */
  border-radius: 4px;
}

.step-content::-webkit-scrollbar-thumb:hover {
  background: #64748b; /* bg-slate-500 */
}
</style>
