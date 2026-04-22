<script setup lang="ts">
import { getConfirmState, handleConfirm, handleCancel } from '../composables/useConfirmDialog';

const state = getConfirmState();

// 获取图标和颜色
function getIconConfig() {
  switch (state.value.options.type) {
    case 'danger':
      return {
        icon: 'M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z',
        color: 'bg-red-500/10 text-red-500',
        buttonClass: 'bg-red-600 hover:bg-red-700'
      };
    case 'warning':
      return {
        icon: 'M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z',
        color: 'bg-yellow-500/10 text-yellow-500',
        buttonClass: 'bg-yellow-600 hover:bg-yellow-700'
      };
    case 'info':
    default:
      return {
        icon: 'M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z',
        color: 'bg-blue-500/10 text-blue-500',
        buttonClass: 'bg-blue-600 hover:bg-blue-700'
      };
  }
}
</script>

<template>
  <Teleport to="body">
    <Transition name="confirm">
      <div v-if="state.show" class="fixed inset-0 z-[100] flex items-center justify-center bg-black/60 backdrop-blur-sm">
        <div class="bg-slate-900 border border-slate-700 rounded-xl shadow-2xl max-w-md w-full mx-4 animate-in fade-in zoom-in-95 duration-200">
          <!-- 标题栏 -->
          <div class="p-6 border-b border-slate-800">
            <div class="flex items-start gap-3">
              <div :class="['flex-shrink-0 w-10 h-10 rounded-full flex items-center justify-center', getIconConfig().color]">
                <svg xmlns="http://www.w3.org/2000/svg" class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" :d="getIconConfig().icon" />
                </svg>
              </div>
              <div class="flex-1">
                <h3 class="text-lg font-semibold text-slate-100">{{ state.options.title }}</h3>
                <p class="mt-2 text-sm text-slate-400 whitespace-pre-line">{{ state.options.message }}</p>
              </div>
            </div>
          </div>
          
          <!-- 复选框选项 -->
          <div v-if="state.options.checkboxLabel" class="px-6 py-3">
            <label class="flex items-center gap-2 cursor-pointer">
              <input 
                type="checkbox" 
                v-model="state.checkboxValue"
                class="w-4 h-4 rounded border-slate-600 bg-slate-800 text-blue-600 focus:ring-blue-500 focus:ring-offset-slate-900"
              />
              <span class="text-sm text-slate-300">{{ state.options.checkboxLabel }}</span>
            </label>
          </div>
          
          <!-- 按钮栏 -->
          <div class="p-6 border-t border-slate-800 flex justify-end gap-3">
            <button 
              @click="handleCancel" 
              class="px-5 py-2 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg font-medium transition text-slate-300"
            >
              {{ state.options.cancelText }}
            </button>
            <button 
              @click="handleConfirm" 
              :class="['px-5 py-2 rounded-lg font-medium transition text-white', getIconConfig().buttonClass]"
            >
              {{ state.options.confirmText }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.confirm-enter-active,
.confirm-leave-active {
  transition: opacity 0.2s ease;
}

.confirm-enter-from,
.confirm-leave-to {
  opacity: 0;
}

.confirm-enter-active .animate-in,
.confirm-leave-active .animate-in {
  animation: none;
}
</style>
