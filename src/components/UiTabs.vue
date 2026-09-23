<script setup lang="ts">
/**
 * 下划线式标签页：选中项用底部分割线上的蓝色指示条标识，
 * 未选中项无边框，避免亮色模式下「按钮描边对比不足」的问题。
 */
export interface UiTabItem {
  id: string;
  label: string;
  /** 可选角标，如镜像源「已覆盖」 */
  badge?: string;
}

withDefaults(
  defineProps<{
    modelValue: string;
    items: UiTabItem[];
    /** md=页面一级；sm=内容区内二级分类 */
    size?: 'md' | 'sm';
  }>(),
  { size: 'md' },
);

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void;
}>();
</script>

<template>
  <div
    role="tablist"
    class="flex overflow-x-auto scrollbar-hide border-b border-slate-200 dark:border-slate-800"
  >
    <button
      v-for="item in items"
      :key="item.id"
      type="button"
      role="tab"
      :aria-selected="modelValue === item.id"
      @click="emit('update:modelValue', item.id)"
      :class="[
        'relative shrink-0 font-medium transition-colors whitespace-nowrap border-b-2 -mb-px',
        size === 'sm' ? 'px-3 py-2 text-xs sm:text-sm' : 'px-4 py-2.5 text-sm sm:text-base',
        modelValue === item.id
          ? 'border-blue-600 text-blue-600 dark:border-blue-400 dark:text-blue-400'
          : 'border-transparent text-slate-500 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-200 hover:border-slate-300 dark:hover:border-slate-600',
      ]"
    >
      <span>{{ item.label }}</span>
      <span
        v-if="item.badge"
        class="ml-1 text-[10px] sm:text-xs opacity-70"
        :title="item.badge"
      >{{ item.badge }}</span>
    </button>
  </div>
</template>
