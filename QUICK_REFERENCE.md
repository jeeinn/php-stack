# PHP-Stack V2.0 快速参考指南

## 📚 文档导航

| 文档 | 用途 |
|------|------|
| [README.md](./README.md) | 项目介绍、安装指南、功能特性 |
| [AGENTS.md](./AGENTS.md) | AI Agent 开发指南、架构说明、后续任务 |
| [IMPLEMENTATION_SUMMARY.md](./IMPLEMENTATION_SUMMARY.md) | V2.0 实现总结、测试统计、代码质量报告 |
| [QUICKSTART-V2.md](./QUICKSTART-V2.md) | V2.0 快速启动指南（如存在） |

---

## 🎯 V2.0 核心功能速览

### 1. 可视化环境配置
**入口**: 侧边栏 → ⚙️ 环境配置  
**文件**: `src/components/EnvConfigPage.vue`

**使用流程**:
1. 选择服务类型（PHP、MySQL、Redis、Nginx）
2. 配置版本号和端口
3. 选择 PHP 扩展（可选）
4. 点击"预览配置"查看生成的 `.env` 和 `docker-compose.yml`
5. 点击"应用配置"生成文件并创建目录结构

**关键 API**:
```typescript
// 验证配置
await invoke('validate_env_config', { config })

// 生成 .env 预览
const envContent = await invoke('generate_env_config', { config })

// 生成 docker-compose.yml 预览
const composeContent = await invoke('preview_compose', { config })

// 应用配置（写入文件）
await invoke('apply_env_config', { config })
```

---

### 2. 统一镜像源管理
**入口**: 侧边栏 → 🌐 镜像源  
**文件**: `src/components/MirrorPanel.vue`

**预设方案**:
- 阿里云全套
- 清华大学全套
- 腾讯云全套
- 中科大全套
- 官方默认源

**使用流程**:
1. 选择预设方案或单独配置各类别
2. 点击"测试连接"验证镜像源可用性
3. 点击"应用"保存配置到 `.env`

**关键 API**:
```typescript
// 获取预设方案列表
const presets = await invoke('get_mirror_presets')

// 应用预设方案
await invoke('apply_mirror_preset', { preset_name: '阿里云全套' })

// 更新单个类别
await invoke('update_single_mirror', { 
  category: 'apt', 
  source: 'https://mirrors.tuna.tsinghua.edu.cn/debian/' 
})

// 测试连接
const isOk = await invoke('test_mirror', { url: '...' })

// 获取当前状态
const status = await invoke('get_mirror_status')
```

---

### 3. 环境备份
**入口**: 侧边栏 → 💾 备份  
**文件**: `src/components/BackupPage.vue`

**备份内容**:
- ✅ `.env` 配置文件
- ✅ `docker-compose.yml`
- ✅ `services/` 目录（Dockerfile 和配置）
- 🔄 数据库导出（mysqldump，当前为占位符）
- 🔄 项目文件（glob 模式匹配）
- 🔄 Nginx vhost 配置
- 🔄 最近 7 天日志

**使用流程**:
1. 选择备份选项（数据库、项目文件等）
2. 选择保存路径（使用 Tauri dialog）
3. 点击"创建备份"
4. 观察进度条和日志
5. 查看备份结果摘要

**关键 API**:
```typescript
import { listen } from '@tauri-apps/api/event'

// 监听备份进度
const unlisten = await listen('backup-progress', (event) => {
  console.log(`步骤: ${event.payload.step}, 进度: ${event.payload.percentage}%`)
})

// 创建备份
await invoke('create_backup', {
  save_path: '/path/to/backup.zip',
  options: {
    include_database: true,
    include_projects: false,
    project_patterns: ['**/*.env'],
    include_vhosts: true,
    include_logs: false
  }
})
```

---

### 4. 环境恢复
**入口**: 侧边栏 → 📥 恢复  
**文件**: `src/components/RestorePage.vue`

**恢复流程**:
1. 选择备份 ZIP 文件
2. 点击"预览"查看备份内容
3. 点击"验证完整性"检查 SHA256
4. 处理端口冲突（如有）
5. 点击"开始恢复"
6. 观察进度和结果

**关键 API**:
```typescript
// 预览备份内容
const preview = await invoke('preview_restore', { 
  zip_path: '/path/to/backup.zip' 
})
console.log('服务列表:', preview.manifest.services)
console.log('端口冲突:', preview.port_conflicts)

// 验证完整性
const isValid = await invoke('verify_backup', { 
  zip_path: '/path/to/backup.zip' 
})

// 执行恢复
await invoke('execute_restore', {
  zip_path: '/path/to/backup.zip',
  port_overrides: { 'mysql': 3307 } // 端口覆盖映射
})
```

---

## 🏗️ 后端模块架构

### 引擎模块 (`src-tauri/src/engine/`)

| 模块 | 行数 | 职责 |
|------|------|------|
| `env_parser.rs` | 495 | .env 文件解析与格式化 |
| `config_generator.rs` | 651 | 配置生成（.env + docker-compose.yml） |
| `mirror_manager.rs` | ~400 | 统一镜像源管理 |
| `backup_manifest.rs` | 273 | 备份清单数据模型 |
| `backup_engine.rs` | 368 | 备份引擎（ZIP 打包） |
| `restore_engine.rs` | 625 | 恢复引擎（ZIP 解压） |

### Tauri 命令 (`src-tauri/src/commands.rs`)

#### 配置生成命令
- `validate_env_config` - 验证配置
- `generate_env_config` - 生成 .env 预览
- `preview_compose` - 预览 docker-compose.yml
- `apply_env_config` - 应用配置

#### 镜像源命令
- `get_mirror_presets` - 获取预设方案
- `apply_mirror_preset` - 应用预设
- `update_single_mirror` - 更新单个类别
- `test_mirror` - 测试连接
- `get_mirror_status` - 获取当前状态

#### 备份命令
- `create_backup` - 创建备份

#### 恢复命令
- `preview_restore` - 预览备份
- `verify_backup` - 验证完整性
- `execute_restore` - 执行恢复

---

## 🧪 测试指南

### 运行所有测试
```bash
cd src-tauri
cargo test --lib
```

### 运行特定模块测试
```bash
# Env 解析器测试
cargo test env_parser

# 配置生成器测试
cargo test config_generator

# 备份引擎测试
cargo test backup

# 恢复引擎测试
cargo test restore_engine

# 镜像源管理测试
cargo test mirror_manager
```

### 属性测试说明
使用 `proptest` crate 进行随机化测试，每个属性测试运行 100+ 次迭代：

```rust
proptest! {
    #[test]
    fn test_property_name(input in any_valid_input()) {
        // 测试逻辑
    }
}
```

**已实现的属性测试**:
- Property 1-6: 配置生成正确性
- Property 7: 镜像源类别独立性
- Property 8: SHA256 完整性验证
- Property 9-10: Env_File 往返一致性
- Property 11-12: Manifest 往返一致性

---

## 📝 开发规范速查

### Rust 后端

#### 错误处理
```rust
// 暴露给前端的命令必须返回 Result<T, String>
#[tauri::command]
pub async fn my_command() -> Result<(), String> {
    do_something().map_err(|e| format!("操作失败: {}", e))
}
```

#### 异步处理
```rust
// Docker 操作必须使用 async/await
pub async fn list_containers() -> Result<Vec<Container>, String> {
    let manager = DockerManager::new()?;
    manager.list_ps_containers().await.map_err(|e| e.to_string())
}
```

#### 模块注册
```rust
// 在 src-tauri/src/engine/mod.rs 中声明
pub mod my_new_module;
```

#### 命令注册
```rust
// 在 src-tauri/src/lib.rs 中注册
.invoke_handler(tauri::generate_handler![
    my_new_command,
    // ... 其他命令
])
```

### Vue 前端

#### 类型定义
```typescript
// 在 src/types/ 目录下定义
export interface MyType {
  field1: string;
  field2: number;
}
```

#### Tauri 调用
```typescript
import { invoke } from '@tauri-apps/api/core'

const result = await invoke('my_command', { param: value })
```

#### 事件监听
```typescript
import { listen } from '@tauri-apps/api/event'

const unlisten = await listen('my-event', (event) => {
  console.log(event.payload)
})

// 组件卸载时取消监听
onUnmounted(() => {
  unlisten()
})
```

#### Tailwind CSS
```vue
<style scoped>
@reference "tailwindcss";

.my-class {
  @apply px-4 py-2 bg-blue-600 text-white rounded;
}
</style>
```

---

## 🔍 常见问题

### Q1: 如何添加新的服务类型？
**A**: 修改以下文件：
1. `src-tauri/src/engine/config_generator.rs` - 添加 `ServiceType` 枚举值
2. `src/types/env-config.ts` - 更新 TypeScript 类型
3. `src/components/EnvConfigPage.vue` - 添加 UI 选项

### Q2: 如何添加新的镜像源预设？
**A**: 修改 `src-tauri/src/engine/mirror_manager.rs` 中的 `get_presets()` 函数，添加新的 `MirrorPreset`。

### Q3: 如何实现完整的 mysqldump？
**A**: 在 `backup_engine.rs` 中使用 `bollard` 的 exec API：
```rust
use bollard::exec::{CreateExecOptions, StartExecResults};

let exec = docker.create_exec(
    &container_name,
    CreateExecOptions {
        cmd: Some(vec!["mysqldump", "-u", "root", "--all-databases"]),
        ..Default::default()
    }
).await?;

docker.start_exec(&exec.id, None).await?;
```

### Q4: 如何处理大文件备份？
**A**: 当前实现使用内存缓冲，建议：
1. 使用流式压缩（zip crate 支持）
2. 分块读取大文件
3. 设置文件大小限制

### Q5: 如何调试 Tauri 应用？
**A**: 
```bash
# 前端调试
npm run tauri dev

# 后端调试（Rust）
cd src-tauri
cargo build
# 使用 rust-gdb 或 rust-lldb

# 查看日志
# Windows: %APPDATA%\com.php-stack.dev\logs\
```

---

## 📊 性能指标

### 构建时间
- Rust 后端: ~30-60 秒（首次），~5-10 秒（增量）
- 前端: ~5-10 秒

### 运行时性能
- 容器列表刷新: < 100ms
- 配置生成: < 50ms
- 备份速度: ~50-100 MB/s（取决于磁盘）
- 恢复速度: ~50-100 MB/s（取决于磁盘）

### 内存占用
- 开发模式: ~200-300 MB
- 生产模式: ~100-150 MB

---

## 🚀 快速开始新任务

### V2.0 版本定位

**重要说明**: PHP-Stack V2.0 是一个**环境配置管理与迁移工具**，专注于：
- ✅ 可视化配置生成（替代手动编辑 .env 和 docker-compose.yml）
- ✅ 镜像源统一管理（加速国内开发体验）
- ✅ 环境备份与恢复（快速迁移开发环境到新机器）

**不包含的功能**:
- ❌ 软件管理中心（多版本一键安装）
- ❌ 虚拟主机管理（Nginx 站点配置）

### 1. 理解需求
- 阅读 `.kiro/specs/` 中的需求和设计文档
- 确认是前端还是后端任务
- **注意**: V2.0 生产版不规划软件管理和虚拟主机功能

### 2. 编写测试
```bash
# Rust 单元测试
cd src-tauri
cargo test

# 前端手动测试
npm run tauri dev
```

### 3. 实现功能
- 后端：修改 `src-tauri/src/` 下的文件
- 前端：修改 `src/` 下的文件

### 4. 更新文档
- 修改 `AGENTS.md`（如有架构变更）
- 修改 `README.md`（如有新功能）

### 5. 验证
```bash
# 运行所有测试
cd src-tauri && cargo test

# 构建前端
cd .. && npm run build
```

---

## 📞 联系与支持

- **项目仓库**: [GitHub](https://github.com/your-repo/php-stack)
- **问题反馈**: GitHub Issues
- **开发文档**: 参见 `AGENTS.md` 和 `IMPLEMENTATION_SUMMARY.md`

---

**最后更新**: 2026-04-17  
**版本**: V2.0  
**维护者**: PHP-Stack Team
