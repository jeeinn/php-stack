<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import { useI18n } from 'vue-i18n';

interface SelectOption {
  value: string;
  label: string;
  disabled?: boolean;
}

interface Props {
  modelValue: string;
  options: SelectOption[];
  placeholder?: string;
  disabled?: boolean;
  size?: 'sm' | 'md' | 'lg';
}

const props = withDefaults(defineProps<Props>(), {
  placeholder: '',
  disabled: false,
  size: 'md',
});

const { t } = useI18n();
const resolvedPlaceholder = computed(() => props.placeholder || t('common.selectPlaceholder'));

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void;
  (e: 'change', value: string): void;
}>();

const isOpen = ref(false);
const selectRef = ref<HTMLElement | null>(null);
// 生成唯一ID用于ARIA属性关联（使用crypto.randomUUID确保全局唯一）
const uniqueId = typeof crypto !== 'undefined' && crypto.randomUUID 
  ? crypto.randomUUID() 
  : Math.random().toString(36).substring(2, 11);

// 尺寸样式映射（常量）
const sizeClasses = {
  sm: 'px-2 py-1 text-xs',
  md: 'px-3 py-2 text-sm',
  lg: 'px-4 py-3 text-base',
} as const;

// 获取当前选中项的显示文本
const selectedLabel = computed(() => {
  const option = props.options.find(opt => opt.value === props.modelValue);
  return option ? option.label : '';
});

// 打开下拉框
function toggleDropdown() {
  if (props.disabled) return;
  isOpen.value = !isOpen.value;
}

// 选择选项
function selectOption(option: SelectOption) {
  if (option.disabled || props.disabled) return;
  
  emit('update:modelValue', option.value);
  emit('change', option.value);
  isOpen.value = false;
}

// 点击外部关闭下拉框
function handleClickOutside(event: MouseEvent) {
  if (selectRef.value && !selectRef.value.contains(event.target as Node)) {
    isOpen.value = false;
  }
}

// 键盘事件处理（绑定到button元素上）
function handleKeydown(event: KeyboardEvent) {
  if (props.disabled) return;
  
  if (event.key === 'Escape') {
    isOpen.value = false;
  } else if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault();
    toggleDropdown();
  } else if (isOpen.value) {
    // 键盘导航：箭头键只更新选中值（预览），不触发change事件
    // Enter或点击选项时才会同时触发update:modelValue和change（确认选择）
    // 这与原生select的行为一致，符合用户预期
    const options = props.options.filter(opt => !opt.disabled);
    const currentIndex = options.findIndex(opt => opt.value === props.modelValue);
    
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      const nextIndex = currentIndex < options.length - 1 ? currentIndex + 1 : 0;
      emit('update:modelValue', options[nextIndex].value);
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      const prevIndex = currentIndex > 0 ? currentIndex - 1 : options.length - 1;
      emit('update:modelValue', options[prevIndex].value);
    } else if (event.key === 'Home') {
      event.preventDefault();
      emit('update:modelValue', options[0].value);
    } else if (event.key === 'End') {
      event.preventDefault();
      emit('update:modelValue', options[options.length - 1].value);
    }
  }
}

onMounted(() => {
  document.addEventListener('click', handleClickOutside);
});

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside);
});

// 监听modelValue变化，确保值在选项中存在
watch(() => props.modelValue, (newValue) => {
  if (newValue && !props.options.some(opt => opt.value === newValue)) {
    console.warn(`Select value "${newValue}" not found in options`);
  }
});
</script>

<template>
  <div 
    ref="selectRef"
    class="relative"
    :class="{ 'opacity-50 cursor-not-allowed': disabled }"
  >
    <!-- 触发器 -->
    <button
      type="button"
      @click="toggleDropdown"
      @keydown="handleKeydown"
      :disabled="disabled"
      role="combobox"
      aria-haspopup="listbox"
      :aria-expanded="isOpen"
      :aria-controls="`select-options-${uniqueId}`"
      class="w-full bg-slate-800 border border-slate-700 rounded-lg outline-none transition-all duration-200 flex items-center justify-between"
      :class="[
        sizeClasses[size],
        isOpen ? 'ring-2 ring-blue-500 border-blue-500' : 'hover:border-slate-600',
        disabled ? 'cursor-not-allowed' : 'cursor-pointer'
      ]"
    >
      <span 
        class="truncate"
        :class="selectedLabel ? 'text-slate-200' : 'text-slate-500'"
      >
        {{ selectedLabel || resolvedPlaceholder }}
      </span>
      <svg 
        xmlns="http://www.w3.org/2000/svg" 
        width="16" 
        height="16" 
        viewBox="0 0 24 24" 
        fill="none" 
        stroke="currentColor" 
        stroke-width="2" 
        stroke-linecap="round" 
        stroke-linejoin="round"
        class="text-slate-400 transition-transform duration-200 flex-shrink-0 ml-2"
        :class="{ 'rotate-180': isOpen }"
      >
        <polyline points="6 9 12 15 18 9"></polyline>
      </svg>
    </button>

    <!-- 下拉选项列表 -->
    <Transition
      enter-active-class="transition ease-out duration-200"
      enter-from-class="transform opacity-0 scale-95"
      enter-to-class="transform opacity-100 scale-100"
      leave-active-class="transition ease-in duration-150"
      leave-from-class="transform opacity-100 scale-100"
      leave-to-class="transform opacity-0 scale-95"
    >
      <div 
        v-if="isOpen" 
        :id="`select-options-${uniqueId}`"
        role="listbox"
        class="absolute z-50 mt-1 w-full bg-slate-800 border border-slate-700 rounded-lg shadow-xl max-h-60 overflow-y-auto scrollbar-hide"
      >
        <div 
          v-for="option in options" 
          :key="option.value"
          @click="selectOption(option)"
          role="option"
          :aria-selected="option.value === modelValue"
          class="px-3 py-2 text-sm cursor-pointer transition-colors duration-150"
          :class="[
            option.value === modelValue 
              ? 'bg-blue-600/20 text-blue-400' 
              : 'text-slate-300 hover:bg-slate-700',
            option.disabled ? 'opacity-50 cursor-not-allowed' : '',
            size === 'sm' ? 'text-xs py-1.5' : size === 'lg' ? 'text-base py-2.5' : ''
          ]"
        >
          {{ option.label }}
        </div>
        
        <!-- 空状态 -->
        <div 
          v-if="options.length === 0" 
          class="px-3 py-4 text-center text-slate-500 text-sm"
        >
          {{ $t('common.noOptions') }}
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
@reference "tailwindcss";

.scrollbar-hide::-webkit-scrollbar {
  display: none;
}

.scrollbar-hide {
  -ms-overflow-style: none;
  scrollbar-width: none;
}
</style>
