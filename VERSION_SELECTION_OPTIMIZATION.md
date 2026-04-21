# 版本选择界面优化说明

## 📋 优化目标

让用户在选择 Docker 镜像版本时，能够清楚地看到**版本号**和**完整镜像标签**的对应关系，避免隐藏转换。

## ✅ 已完成的优化

### 1. 类型定义更新

**文件**: `src/types/env-config.ts`

新增 `VersionInfo` 接口：
```typescript
export interface VersionInfo {
  version: string;        // 版本号（如 "7.2", "1.27"）
  tag: string;            // 完整标签（如 "7.2-alpine", "1.27-alpine"）
  full_name: string;      // 完整镜像名（如 "redis:7.2-alpine"）
  eol: boolean;           // 是否已停止维护
  description?: string;   // 版本描述
  has_user_override?: boolean; // 是否有用户自定义覆盖
}
```

### 2. 前端数据结构统一

**文件**: `src/components/EnvConfigPage.vue`

- 将所有版本列表从 `string[]` 改为 `VersionInfo[]`
- 统一 PHP、MySQL、Redis、Nginx 的数据结构
- 后端返回的完整版本信息直接使用前端的 `VersionInfo` 类型

### 3. 界面展示优化

#### 下拉选项显示格式
```
Redis 7.2 → 7.2-alpine (EOL)
```

- **左侧**: 简洁的版本号（用户友好）
- **右侧**: 完整的 Docker 标签（技术准确）
- **标记**: EOL 版本用橙色标注

#### 实时镜像预览
在每个版本选择框上方显示：
```
Redis 版本 (将使用镜像: redis:7.2-alpine)
```

让用户在选择前就能清楚知道会使用哪个 Docker 镜像。

### 4. 辅助函数

添加了 4 个辅助函数来获取完整镜像名：
- `getPhpImageTag(version)` → `php:8.2-fpm`
- `getMysqlImageTag(version)` → `mysql:8.0`
- `getRedisImageTag(version)` → `redis:7.2-alpine`
- `getNginxImageTag(version)` → `nginx:1.27-alpine`

这些函数会：
1. 从版本列表中查找对应的 `VersionInfo`
2. 返回 `full_name` 字段
3. 如果找不到，返回默认格式作为后备

## 🎯 用户体验改进

### 优化前
```
下拉框显示: redis:7.2-alpine
问题: 用户不知道 "7.2-alpine" 是什么版本
```

### 优化后
```
标签提示: Redis 版本 (将使用镜像: redis:7.2-alpine)
下拉选项: Redis 7.2 → 7.2-alpine
优势: 
  ✓ 版本号清晰（7.2）
  ✓ 完整标签可见（7.2-alpine）
  ✓ 完整镜像名预览（redis:7.2-alpine）
  ✓ EOL 版本明确标注
```

## 🔧 技术实现细节

### 数据流
```
version_manifest.json (Rust)
    ↓
VersionManifest::new() (编译时嵌入)
    ↓
get_version_mappings() (Tauri Command)
    ↓
前端 loadVersionMappings()
    ↓
VersionInfo[] 数组
    ↓
UI 下拉选项 + 实时预览
```

### 关键修改点

1. **loadVersionMappings()**: 直接使用后端返回的完整对象，不再手动映射
2. **ensureVersionInList()**: 改为基于 `version` 字段查找，而不是字符串比较
3. **addXxxVersion()**: 从 `VersionInfo` 对象中提取 `version` 字段
4. **模板渲染**: 使用 `v.version` 作为 key 和 value，显示 `v.tag`

## ✨ 额外优势

1. **支持用户自定义覆盖**: `has_user_override` 字段可用于在 UI 中标记自定义版本
2. **EOL 警告**: 自动显示已停止维护的版本
3. **版本描述**: 可以显示详细的版本说明（如 "PHP 8.2 (活跃支持)"）
4. **一致性**: 所有服务类型的处理方式完全一致

## 🧪 测试验证

- ✅ TypeScript 编译通过
- ✅ Vite 构建成功
- ✅ Rust 单元测试全部通过（7/7）
- ✅ 无类型错误

## 📝 后续可扩展功能

1. 在下拉选项中显示 `description` 字段
2. 为有用户覆盖的版本添加特殊图标（✨）
3. 在预览配置时显示所有服务的完整镜像名
4. 添加版本对比功能（显示不同版本的特性差异）

---

**优化日期**: 2026-04-21  
**涉及文件**: 
- `src/types/env-config.ts`
- `src/components/EnvConfigPage.vue`
