# 开发阶段日志规范

## 📋 概述

在开发阶段，保留清晰、友好的日志输出对于问题排查和用户体验至关重要。本文档定义了项目的日志规范和最佳实践。

---

## 🎯 设计原则

### 1. 用户友好

- ✅ 使用 emoji 增强可读性
- ✅ 清晰的步骤提示
- ✅ 区分信息、警告、错误

### 2. 问题排查

- ✅ 关键路径必须记录
- ✅ 配置加载过程透明
- ✅ 错误信息详细具体

### 3. 双重输出

- 🖥️ **终端输出**: 开发者查看
- 🌐 **前端UI**: 用户实时看到

---

## 📝 日志级别与格式

### INFO - 正常流程

**格式**: `emoji [模块] 消息`

**示例**:
```rust
eprintln!("📝 [UserOverride] 加载用户覆盖配置");
eprintln!("✅ [ConfigGenerator] 配置应用成功");
```

**常用 Emoji**:
- 📝 - 开始操作
- ✅ - 成功完成
- ℹ️ - 提示信息
- 🔧 - 处理中
- 📁 - 文件/目录操作
- 🐳 - Docker 相关

### WARN - 警告信息

**格式**: `⚠️  [模块] 警告消息`

**示例**:
```rust
eprintln!("⚠️  [UserOverride] 解析配置文件失败: {}", e);
```

### ERROR - 错误信息

**格式**: `❌ [模块] 错误消息`

**示例**:
```rust
eprintln!("❌ [UserOverride] 读取配置文件失败: {}", e);
```

---

## 🔑 关键日志点

### 1. 用户覆盖配置加载

**位置**: `user_override_manager.rs::load_user_overrides()`

**日志内容**:
```rust
// 文件不存在
eprintln!("ℹ️  [UserOverride] 未找到用户覆盖配置文件，使用默认配置");

// 开始加载
eprintln!("📝 [UserOverride] 加载用户覆盖配置: {:?}", overrides_path);

// 加载成功
eprintln!("✅ [UserOverride] 加载成功，共 {} 个服务类型，{} 个版本覆盖", 
    result.len(), override_count);

// 每个服务的详情
eprintln!("   ✅ {}: {} 个版本覆盖", service_key, versions.len());

// 解析失败
eprintln!("⚠️  [UserOverride] 解析配置文件失败: {}", e);

// 读取失败
eprintln!("❌ [UserOverride] 读取配置文件失败: {}", e);
```

### 2. 配置合并

**位置**: `user_override_manager.rs::get_merged_image_info()`

**日志内容**:
```rust
// 使用用户覆盖
eprintln!("🔧 [UserOverride] {} {} 使用自定义标签: {}", 
    service_type, version, user_override.tag);
```

### 3. 配置应用流程

**位置**: `commands.rs::apply_env_config()`

**日志内容**:
```rust
emit_log("📝 开始应用配置...");
emit_log(&format!("📁 项目根目录: {:?}", project_root));

// 检查用户覆盖
if overrides_path.exists() {
    emit_log("✅ 检测到用户版本覆盖配置");
} else {
    emit_log("ℹ️  未找到用户覆盖配置，使用默认配置");
}

// 详细步骤
emit_log("🔧 验证配置...");
emit_log("📄 生成 .env 文件...");
emit_log("🐳 生成 docker-compose.yml...");
emit_log("📂 创建服务目录结构...");

// 完成
emit_log("✅ 配置应用成功！");
emit_log("💡 提示：请重启容器使新配置生效");

// 失败
emit_log(&format!("❌ 配置应用失败: {}", e));
```

---

## 🌐 前端实时日志

### Tauri 事件机制

后端通过 `app_handle.emit()` 发送日志到前端：

```rust
use tauri::Emitter;

let emit_log = |msg: &str| {
    eprintln!("{}", msg);              // 终端输出
    let _ = app_handle.emit("env-log", msg); // 前端UI显示
};
```

### 前端接收

在 Vue 组件中监听事件：

```typescript
import { listen } from '@tauri-apps/api/event';

onMounted(async () => {
  const unlisten = await listen<string>('env-log', (event) => {
    logs.value.push({
      timestamp: new Date().toLocaleTimeString(),
      message: event.payload
    });
  });
  
  onUnmounted(() => unlisten());
});
```

### UI 展示

```vue
<template>
  <div class="log-container">
    <div v-for="(log, index) in logs" :key="index" class="log-item">
      <span class="timestamp">{{ log.timestamp }}</span>
      <span class="message">{{ log.message }}</span>
    </div>
  </div>
</template>

<style scoped>
.log-container {
  background: #1e1e1e;
  color: #d4d4d4;
  padding: 1rem;
  border-radius: 8px;
  font-family: 'Consolas', monospace;
  max-height: 400px;
  overflow-y: auto;
}

.log-item {
  margin: 4px 0;
  line-height: 1.5;
}

.timestamp {
  color: #6a9955;
  margin-right: 8px;
}
</style>
```

---

## 📊 日志示例

### 正常流程

```
📝 开始应用配置...
📁 项目根目录: "E:\\study\\php-stack"
✅ 检测到用户版本覆盖配置
🔧 验证配置...
📄 生成 .env 文件...
🐳 生成 docker-compose.yml...
📂 创建服务目录结构...
📝 [UserOverride] 加载用户覆盖配置: "E:\\study\\php-stack\\.user_version_overrides.json"
   ✅ redis: 1 个版本覆盖
✅ [UserOverride] 加载成功，共 1 个服务类型，1 个版本覆盖
🔧 [UserOverride] redis 6.2 使用自定义标签: 6.2-alpine-01
✅ 配置应用成功！
💡 提示：请重启容器使新配置生效
```

### 无用户覆盖

```
📝 开始应用配置...
📁 项目根目录: "E:\\study\\php-stack"
ℹ️  未找到用户覆盖配置，使用默认配置
🔧 验证配置...
📄 生成 .env 文件...
🐳 生成 docker-compose.yml...
📂 创建服务目录结构...
ℹ️  [UserOverride] 未找到用户覆盖配置文件，使用默认配置
✅ 配置应用成功！
💡 提示：请重启容器使新配置生效
```

### 错误情况

```
📝 开始应用配置...
📁 项目根目录: "E:\\study\\php-stack"
✅ 检测到用户版本覆盖配置
🔧 验证配置...
📄 生成 .env 文件...
⚠️  [UserOverride] 解析配置文件失败: expected `,` or `}` at line 5 column 3
❌ 配置应用失败: 配置文件格式错误
```

---

## 🛠️ 最佳实践

### 1. 保持一致性

所有日志使用统一的格式：
```rust
// ✅ 正确
eprintln!("✅ [Module] 操作成功");

// ❌ 错误
println!("Success");
eprintln!("[APPLY_CONFIG] done");
```

### 2. 提供上下文

错误信息应包含足够的上下文：
```rust
// ✅ 好
eprintln!("❌ [UserOverride] 读取配置文件失败: {}", e);

// ❌ 差
eprintln!("Error reading file");
```

### 3. 避免过度日志

只记录关键路径：
```rust
// ✅ 必要
eprintln!("📝 [UserOverride] 加载用户覆盖配置");

// ❌ 过度
eprintln!("Reading file...");
eprintln!("File opened");
eprintln!("Parsing JSON...");
eprintln!("JSON parsed");
```

### 4. 生产环境考虑

未来可以通过条件编译控制日志：

```rust
#[cfg(debug_assertions)]
eprintln!("📝 [UserOverride] 加载用户覆盖配置");

// 或使用日志库
use log::{info, warn, error};

info!("[UserOverride] 加载用户覆盖配置");
warn!("[UserOverride] 解析失败: {}", e);
error!("[UserOverride] 读取失败: {}", e);
```

---

## 📚 相关模块

### 后端日志

| 模块 | 文件 | 日志内容 |
|------|------|---------|
| UserOverrideManager | `user_override_manager.rs` | 配置加载、合并 |
| ConfigGenerator | `config_generator.rs` | 配置生成（待添加） |
| Commands | `commands.rs` | 应用流程、步骤 |

### 前端日志

| 组件 | 文件 | 功能 |
|------|------|------|
| EnvConfigPage | `EnvConfigPage.vue` | 显示配置应用日志 |
| 日志监听 | `listen('env-log')` | 接收后端事件 |

---

## 🚀 未来优化

### 短期
1. ✅ ~~增强用户覆盖配置日志~~
2. ✅ ~~优化配置应用流程日志~~
3. ⏳ 添加 ConfigGenerator 详细步骤日志

### 中期
1. 🔧 引入 `log` crate 替代 `eprintln!`
2. 📝 添加日志级别过滤（DEBUG/INFO/WARN/ERROR）
3. 🎨 前端日志 UI 美化（颜色、折叠等）

### 长期
1. 🚀 支持日志导出功能
2. 📊 添加日志分析工具
3. 🔍 集成 Sentry 等错误追踪服务

---

## 📖 参考资料

- [Tauri Event System](https://tauri.app/v1/guides/features/events/)
- [Rust log crate](https://docs.rs/log/)
- [Emoji 列表](https://emojipedia.org/)

---

**最后更新**: 2026-04-20  
**维护者**: PHP-Stack Team
