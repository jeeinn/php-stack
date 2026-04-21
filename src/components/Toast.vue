<script setup lang="ts">
import { getToasts, removeToast } from '../composables/useToast';
import { computed } from 'vue';

const toasts = getToasts();

// 获取样式类
function getToastClass(type: string): string {
  switch (type) {
    case 'success':
      return 'bg-emerald-500/10 border-emerald-500/20 text-emerald-400';
    case 'error':
      return 'bg-rose-500/10 border-rose-500/20 text-rose-400';
    case 'warning':
      return 'bg-amber-500/10 border-amber-500/20 text-amber-400';
    case 'info':
    default:
      return 'bg-blue-500/10 border-blue-500/20 text-blue-400';
  }
}

// 获取图标
function getIcon(type: string): string {
  switch (type) {
    case 'success':
      return '✓';
    case 'error':
      return 'E';
    case 'warning':
      return '⚠';
    case 'info':
    default:
      return 'ℹ';
  }
}
</script>

<template>
  <Teleport to="body">
    <div class="fixed top-4 left-1/2 -translate-x-1/2 z-[9999] flex flex-col gap-2 items-center pointer-events-none">
      <TransitionGroup name="toast">
        <div
          v-for="toast in toasts"
          :key="toast.id"
          :class="[
            'pointer-events-auto px-6 py-3 rounded-xl border shadow-lg backdrop-blur-sm min-w-[300px] max-w-[600px] flex items-center gap-3',
            getToastClass(toast.type)
          ]"
        >
          <span class="text-lg flex-shrink-0">{{ getIcon(toast.type) }}</span>
          <span class="text-sm flex-1 whitespace-pre-wrap">{{ toast.message }}</span>
          <button
            @click="removeToast(toast.id)"
            class="flex-shrink-0 opacity-60 hover:opacity-100 transition"
          >
            ✕
          </button>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<style scoped>
.toast-enter-active,
.toast-leave-active {
  transition: all 0.3s ease;
}

.toast-enter-from {
  opacity: 0;
  transform: translateY(-20px);
}

.toast-leave-to {
  opacity: 0;
  transform: translateY(-20px);
}

.toast-move {
  transition: transform 0.3s ease;
}
</style>
