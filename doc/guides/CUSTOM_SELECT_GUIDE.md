# CustomSelect 组件使用指南

## 概述

`CustomSelect` 是一个自定义的下拉选择组件，完全符合 PHP-Stack 项目的 UI 风格，替代了原生 `<select>` 元素。

## 特性

- ✅ 完全自定义样式，与项目 UI 风格一致
- ✅ 支持键盘导航（Enter/Space 打开，Escape 关闭）
- ✅ 点击外部自动关闭
- ✅ 平滑的动画过渡效果
- ✅ 支持三种尺寸（sm/md/lg）
- ✅ 支持禁用状态
- ✅ 支持空状态提示
- ✅ 完整的 TypeScript 类型支持

## 基本用法

```vue
<script setup lang="ts">
import { ref } from 'vue';
import CustomSelect from '@/components/CustomSelect.vue';

const selectedValue = ref('');
const options = [
  { value: 'option1', label: '选项1' },
  { value: 'option2', label: '选项2' },
  { value: 'option3', label: '选项3' },
];
</script>

<template>
  <CustomSelect
    v-model="selectedValue"
    :options="options"
    placeholder="请选择..."
  />
</template>
```

## Props

| 属性 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `modelValue` | `string` | `''` | 当前选中的值（v-model） |
| `options` | `SelectOption[]` | `[]` | 选项列表 |
| `placeholder` | `string` | `'请选择...'` | 占位符文本 |
| `disabled` | `boolean` | `false` | 是否禁用 |
| `size` | `'sm' \| 'md' \| 'lg'` | `'md'` | 组件尺寸 |

## Events

| 事件名 | 参数 | 说明 |
|--------|------|------|
| `update:modelValue` | `(value: string)` | 选中值变化时触发 |
| `change` | `(value: string)` | 选中值变化时触发（与 update:modelValue 同时触发） |

## SelectOption 类型定义

```typescript
interface SelectOption {
  value: string;      // 选项的值
  label: string;      // 选项的显示文本
  disabled?: boolean; // 是否禁用该选项
}
```

## 使用示例

### 1. 基础用法

```vue
<CustomSelect
  v-model="selectedVersion"
  :options="versionOptions"
  placeholder="选择版本"
/>
```

### 2. 带禁用选项

```vue
<script setup>
const options = [
  { value: 'v1', label: '版本 1.0' },
  { value: 'v2', label: '版本 2.0 (推荐)' },
  { value: 'v3', label: '版本 3.0', disabled: true }, // 禁用选项
];
</script>

<CustomSelect
  v-model="selected"
  :options="options"
/>
```

### 3. 不同尺寸

```vue
<!-- 小尺寸 -->
<CustomSelect
  v-model="value"
  :options="options"
  size="sm"
/>

<!-- 中等尺寸（默认） -->
<CustomSelect
  v-model="value"
  :options="options"
  size="md"
/>

<!-- 大尺寸 -->
<CustomSelect
  v-model="value"
  :options="options"
  size="lg"
/>
```

### 4. 监听变化事件

```vue
<script setup>
import { ref } from 'vue';
import CustomSelect from '@/components/CustomSelect.vue';

const selected = ref('');

function handleChange(value: string) {
  console.log('选中的值:', value);
  // 执行其他逻辑...
}
</script>

<template>
  <CustomSelect
    v-model="selected"
    :options="options"
    @change="handleChange"
  />
</template>
```

### 5. 动态选项（从 API 加载）

```vue
<script setup>
import { ref, onMounted } from 'vue';
import CustomSelect from '@/components/CustomSelect.vue';

const versions = ref([]);
const selected = ref('');

onMounted(async () => {
  // 从后端加载数据
  const data = await fetchVersions();
  versions.value = data.map(v => ({
    value: v.id,
    label: `${v.name} - ${v.version}`,
    disabled: v.deprecated,
  }));
});
</script>

<template>
  <CustomSelect
    v-model="selected"
    :options="versions"
    placeholder="加载中..."
    :disabled="versions.length === 0"
  />
</template>
```

### 6. 带 EOL 标记的选项

```vue
<script setup>
import { computed } from 'vue';

const phpVersions = [
  { id: 'php82', display_name: 'PHP 8.2', image_tag: 'php:8.2-fpm', eol: false },
  { id: 'php74', display_name: 'PHP 7.4', image_tag: 'php:7.4-fpm', eol: true },
];

const versionOptions = computed(() =>
  phpVersions.map(v => ({
    value: v.id,
    label: `${v.display_name} → ${v.image_tag}${v.eol ? ' (EOL)' : ''}`,
  }))
);
</script>

<template>
  <CustomSelect
    v-model="selectedPhp"
    :options="versionOptions"
  />
</template>
```

## 样式定制

组件使用了 Tailwind CSS 类，可以通过以下方式定制：

### 修改主题色

在 `CustomSelect.vue` 中修改相关颜色类：
- 聚焦环：`ring-blue-500` → `ring-purple-500`
- 选中项背景：`bg-blue-600/20` → `bg-purple-600/20`
- 选中项文字：`text-blue-400` → `text-purple-400`

### 修改边框样式

```vue
<!-- 在模板中修改 button 的 class -->
class="... border-slate-700 ..." 
<!-- 改为 -->
class="... border-gray-600 ..."
```

## 注意事项

1. **选项值唯一性**：确保每个选项的 `value` 是唯一的
2. **响应式更新**：当 `options` 数组变化时，组件会自动更新
3. **空值处理**：如果 `modelValue` 不在 `options` 中，会显示 placeholder
4. **无障碍访问**：组件支持键盘操作，符合无障碍标准

## 与原生 Select 的对比

| 特性 | 原生 Select | CustomSelect |
|------|------------|--------------|
| 样式定制 | ❌ 有限 | ✅ 完全可控 |
| 动画效果 | ❌ 无 | ✅ 平滑过渡 |
| 键盘导航 | ⚠️ 基础 | ✅ 完整支持 |
| 空状态提示 | ❌ 无 | ✅ 有 |
| 禁用单个选项 | ✅ 支持 | ✅ 支持 |
| 搜索功能 | ❌ 无 | 🔜 计划中 |
| 多选 | ✅ 支持 | 🔜 计划中 |

## 未来扩展

计划添加的功能：
- [ ] 搜索过滤功能
- [ ] 多选支持
- [ ] 分组选项支持
- [ ] 虚拟滚动（大数据量优化）
- [ ] 自定义选项渲染插槽

## 相关文件

- 组件源码：`src/components/CustomSelect.vue`
- 测试文件：`src/components/__tests__/CustomSelect.spec.ts`
- 使用示例：`src/components/EnvConfigPage.vue`
