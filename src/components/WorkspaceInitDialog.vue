<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

const isOpen = ref(false);
const currentPath = ref('');
const errorMessage = ref('');

onMounted(async () => {
  try {
    const info = await invoke<any>('get_workspace_info');
    if (!info) {
      isOpen.value = true;
    } else {
      // 可以在这里增加有效性验证，如果无效也弹出向导
      currentPath.value = info.workspace_path;
    }
  } catch (e) {
    console.error('Failed to load workspace info:', e);
    isOpen.value = true;
  }
});

async function selectDirectory() {
  const selected = await open({
    directory: true,
    multiple: false,
  });
  if (selected) {
    currentPath.value = selected as string;
    errorMessage.value = '';
  }
}

async function confirmWorkspace() {
  if (!currentPath.value) {
    errorMessage.value = '请选择一个有效的目录作为工作区';
    return;
  }

  try {
    await invoke('set_workspace_path', { path: currentPath.value });
    isOpen.value = false;
    // 刷新页面或触发重新加载逻辑
    window.location.reload();
  } catch (e) {
    errorMessage.value = e as string;
  }
}
</script>

<template>
  <div v-if="isOpen" class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50">
    <div class="bg-slate-900 border border-slate-700 rounded-xl p-8 max-w-md w-full shadow-2xl">
      <h2 class="text-2xl font-bold text-white mb-4">👋 欢迎使用 PHP-Stack</h2>
      <p class="text-slate-400 mb-6">
        为了开始使用，请指定一个文件夹作为您的<strong>工作目录</strong>。
        所有的环境配置、数据库数据和项目源码都将保存在这里。
      </p>

      <div class="space-y-4">
        <div>
          <label class="block text-sm font-medium text-slate-300 mb-2">工作目录路径</label>
          <div class="flex gap-2">
            <input 
              v-model="currentPath" 
              readonly 
              placeholder="点击右侧按钮选择目录..."
              class="flex-1 bg-slate-800 border border-slate-700 rounded-lg px-4 py-2 text-sm text-slate-200 outline-none"
            />
            <button 
              @click="selectDirectory"
              class="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition"
            >
              浏览
            </button>
          </div>
        </div>

        <div v-if="errorMessage" class="text-rose-400 text-sm bg-rose-500/10 p-3 rounded-lg border border-rose-500/20">
          {{ errorMessage }}
        </div>

        <div class="pt-4 flex justify-end">
          <button 
            @click="confirmWorkspace"
            class="px-6 py-2 bg-emerald-600 hover:bg-emerald-700 text-white rounded-lg font-bold transition shadow-lg shadow-emerald-600/20"
          >
            确认并开始
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
@reference "tailwindcss";
</style>
