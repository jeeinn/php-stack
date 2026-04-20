<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { MirrorPreset } from '../types/env-config';

interface MirrorCategory {
  key: string;
  label: string;
  icon: string;
  value: string;
  testing: boolean;
  testResult: 'idle' | 'success' | 'fail';
}

const presets = ref<MirrorPreset[]>([]);
const selectedPreset = ref('');
const applying = ref(false);
const error = ref<string | null>(null);
const successMsg = ref<string | null>(null);

const categories = ref<MirrorCategory[]>([
  { key: 'docker_registry', label: 'Docker Registry', icon: '🐳', value: '', testing: false, testResult: 'idle' },
  { key: 'apt', label: 'APT / Debian', icon: '📦', value: '', testing: false, testResult: 'idle' },
  { key: 'composer', label: 'Composer', icon: '🎵', value: '', testing: false, testResult: 'idle' },
  { key: 'npm', label: 'NPM', icon: '📗', value: '', testing: false, testResult: 'idle' },
]);

// Mirror source options per category
const mirrorOptions: Record<string, { label: string; url: string }[]> = {
  docker_registry: [
    { label: '官方默认', url: 'https://registry-1.docker.io' },
    { label: '阿里云', url: 'https://registry.cn-hangzhou.aliyuncs.com' },
    { label: '腾讯云', url: 'https://mirror.ccs.tencentyun.com' },
    { label: '中科大', url: 'https://docker.mirrors.ustc.edu.cn' },
  ],
  apt: [
    { label: '官方默认', url: 'http://deb.debian.org' },
    { label: '阿里云', url: 'https://mirrors.aliyun.com' },
    { label: '清华大学', url: 'https://mirrors.tuna.tsinghua.edu.cn' },
    { label: '腾讯云', url: 'https://mirrors.cloud.tencent.com' },
    { label: '中科大', url: 'https://mirrors.ustc.edu.cn' },
  ],
  composer: [
    { label: '官方默认', url: 'https://packagist.org' },
    { label: '阿里云', url: 'https://mirrors.aliyun.com/composer/' },
    { label: '腾讯云', url: 'https://mirrors.cloud.tencent.com/composer/' },
    { label: '华为云', url: 'https://repo.huaweicloud.com/repository/php/' },
  ],
  npm: [
    { label: '官方默认', url: 'https://registry.npmjs.org' },
    { label: '淘宝 (npmmirror)', url: 'https://registry.npmmirror.com' },
    { label: '腾讯云', url: 'https://mirrors.cloud.tencent.com/npm/' },
    { label: '华为云', url: 'https://repo.huaweicloud.com/repository/npm/' },
  ],
};

async function loadPresets() {
  try {
    presets.value = await invoke<MirrorPreset[]>('get_mirror_presets');
  } catch (e) {
    console.error('加载预设失败:', e);
  }
}

async function loadStatus() {
  try {
    const status = await invoke<Record<string, string>>('get_mirror_status');
    for (const cat of categories.value) {
      if (status[cat.key]) {
        cat.value = status[cat.key];
      }
    }
  } catch (e) {
    console.error('加载镜像源状态失败:', e);
  }
}

async function loadCurrentPreset() {
  try {
    const presetName = await invoke<string>('get_current_mirror_preset');
    selectedPreset.value = presetName;
  } catch (e) {
    console.error('加载当前预设失败:', e);
    // 如果检测失败，默认选中“官方默认”
    selectedPreset.value = '官方默认';
  }
}

function applyPresetToCategories() {
  const preset = presets.value.find(p => p.name === selectedPreset.value);
  if (!preset) return;
  for (const cat of categories.value) {
    const key = cat.key as keyof MirrorPreset;
    if (typeof preset[key] === 'string') {
      cat.value = preset[key] as string;
    }
  }
}

async function testConnection(cat: MirrorCategory) {
  cat.testing = true;
  cat.testResult = 'idle';
  try {
    const result = await invoke<boolean>('test_mirror', { url: cat.value });
    cat.testResult = result ? 'success' : 'fail';
  } catch {
    cat.testResult = 'fail';
  } finally {
    cat.testing = false;
  }
}

async function handleApply() {
  applying.value = true;
  error.value = null;
  successMsg.value = null;
  try {
    if (selectedPreset.value) {
      await invoke('apply_mirror_preset', { presetName: selectedPreset.value });
    } else {
      for (const cat of categories.value) {
        if (cat.value) {
          await invoke('update_single_mirror', { category: cat.key, source: cat.value });
        }
      }
    }
    successMsg.value = '镜像源配置已应用';
    setTimeout(() => { successMsg.value = null; }, 3000);
  } catch (e) {
    error.value = e as string;
  } finally {
    applying.value = false;
  }
}

onMounted(() => {
  loadPresets();
  loadStatus();
  loadCurrentPreset();
});
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden">
    <header class="flex justify-between items-center mb-6">
      <div>
        <h1 class="text-3xl font-bold">镜像源管理</h1>
        <p class="text-slate-400 text-sm mt-1">统一管理 Docker、APT、Composer、NPM 镜像源</p>
      </div>
      <button
        @click="handleApply"
        :disabled="applying"
        class="px-5 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg font-medium transition disabled:opacity-50"
      >
        {{ applying ? '应用中...' : '应用配置' }}
      </button>
    </header>

    <!-- Messages -->
    <div v-if="error" class="mb-4 p-4 bg-rose-500/10 border border-rose-500/20 rounded-xl text-rose-400 text-sm">
      {{ error }}
    </div>
    <div v-if="successMsg" class="mb-4 p-4 bg-emerald-500/10 border border-emerald-500/20 rounded-xl text-emerald-400 text-sm">
      {{ successMsg }}
    </div>

    <div class="flex-1 overflow-y-auto pr-2 space-y-6">
      <!-- Preset Selection -->
      <section class="bg-slate-900 border border-slate-800 rounded-xl p-6">
        <h2 class="text-lg font-bold mb-4">🎯 预设方案</h2>
        <div class="flex gap-3">
          <select
            v-model="selectedPreset"
            @change="applyPresetToCategories"
            class="flex-1 bg-slate-800 border border-slate-700 rounded-lg px-4 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500"
          >
            <option value="">自定义配置</option>
            <option v-for="p in presets" :key="p.name" :value="p.name">{{ p.name }}</option>
          </select>
        </div>
        <p class="text-xs text-slate-500 mt-2">选择预设方案将同时设置所有镜像源类别，也可以在下方单独调整。</p>
      </section>

      <!-- Individual Categories -->
      <section
        v-for="cat in categories"
        :key="cat.key"
        class="bg-slate-900 border border-slate-800 rounded-xl p-6"
      >
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-lg font-bold">{{ cat.icon }} {{ cat.label }}</h2>
          <div class="flex items-center gap-2">
            <span
              v-if="cat.testResult === 'success'"
              class="text-xs text-emerald-400 flex items-center gap-1"
            >
              <span class="w-2 h-2 bg-emerald-500 rounded-full"></span> 连接成功
            </span>
            <span
              v-if="cat.testResult === 'fail'"
              class="text-xs text-amber-400 flex items-center gap-1"
            >
              <span class="w-2 h-2 bg-amber-500 rounded-full"></span> 连接失败
            </span>
            <button
              @click="testConnection(cat)"
              :disabled="cat.testing || !cat.value"
              class="text-xs px-3 py-1.5 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg transition disabled:opacity-50"
            >
              {{ cat.testing ? '测试中...' : '测试连接' }}
            </button>
          </div>
        </div>
        <select
          v-model="cat.value"
          class="w-full bg-slate-800 border border-slate-700 rounded-lg px-4 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500"
        >
          <option value="">请选择</option>
          <option
            v-for="opt in mirrorOptions[cat.key]"
            :key="opt.url"
            :value="opt.url"
          >
            {{ opt.label }} — {{ opt.url }}
          </option>
        </select>
        <div v-if="cat.testResult === 'fail'" class="mt-2 text-xs text-amber-400">
          连接测试失败，但不影响保存。建议选择其他可用源。
        </div>
      </section>
    </div>
  </div>
</template>
