# PHP-Stack 系统架构文档

> **版本**: V2.5  
> **最后更新**: 2026-04-23  
> **维护者**: PHP-Stack Team

---

## 📋 目录

- [1. 项目概述](#1-项目概述)
- [2. 系统架构图](#2-系统架构图)
- [3. 核心工作流程](#3-核心工作流程)
- [4. 模块详细说明](#4-模块详细说明)
- [5. 数据流图](#5-数据流图)
- [6. 关键技术决策](#6-关键技术决策)
- [7. 扩展指南](#7-扩展指南)

---

## 1. 项目概述

### 1.1 项目定位

PHP-Stack 是一个基于 **Tauri v2 + Docker** 的跨平台 PHP 开发环境可视化管理工具。

**核心价值**：
- 🎯 **可视化配置** - 替代手动编辑 `.env` 和 `docker-compose.yml`
- 🚀 **镜像源加速** - 统一管理 Docker/APT/Composer/NPM 镜像源
- 💾 **环境备份恢复** - ZIP 格式打包，支持 SHA256 完整性校验
- 🔧 **多版本管理** - 支持 PHP/MySQL/Redis/Nginx 多版本共存

### 1.2 技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| **前端** | Vue 3 + TypeScript | UI 界面 |
| **样式** | Tailwind CSS v4 | 响应式设计 |
| **后端** | Rust (Tauri v2) | 系统级操作 |
| **容器** | Docker + Docker Compose | 环境隔离 |
| **Docker SDK** | bollard | Rust 的 Docker API 客户端 |

---

## 2. 系统架构图

```
┌─────────────────────────────────────────────────────────────┐
│                        前端层 (Vue 3)                        │
├─────────────────────────────────────────────────────────────┤
│  App.vue (主框架)                                            │
│  ├── EnvConfigPage.vue      (环境配置)                       │
│  ├── MirrorPanel.vue          (镜像源管理)                   │
│  ├── BackupPage.vue           (环境备份)                     │
│  ├── RestorePage.vue          (环境恢复)                     │
│  └── SoftwareSettings.vue     (软件设置/版本映射)            │
└──────────────────────┬──────────────────────────────────────┘
                       │ Tauri IPC (invoke)
┌──────────────────────▼──────────────────────────────────────┐
│                      后端层 (Rust/Tauri)                     │
├─────────────────────────────────────────────────────────────┤
│  commands.rs (API 命令入口)                                  │
│  ├── check_docker / list_containers                         │
│  ├── generate_env_config / apply_env_config                 │
│  ├── get_mirror_presets / apply_mirror_preset               │
│  ├── create_backup / preview_restore                        │
│  └── get_version_mappings / validate_version                │
├─────────────────────────────────────────────────────────────┤
│  engine/ (核心业务引擎)                                      │
│  ├── config_generator.rs       (配置生成器)                  │
│  ├── version_manifest.rs       (版本清单管理器)              │
│  ├── user_override_manager.rs  (用户覆盖管理器) ⚠️          │
│  ├── env_parser.rs             (.env 解析器)                │
│  ├── mirror_manager.rs         (镜像源管理器)               │
│  ├── backup_engine.rs          (备份引擎)                   │
│  └── restore_engine.rs         (恢复引擎)                   │
├─────────────────────────────────────────────────────────────┤
│  docker/ (Docker 交互层)                                     │
│  ├── manager.rs                (容器管理)                    │
│  └── mirror.rs                 (镜像源切换)                  │
└──────────────────────┬──────────────────────────────────────┘
                       │ bollard (Docker SDK)
┌──────────────────────▼──────────────────────────────────────┐
│                     Docker Engine                            │
├─────────────────────────────────────────────────────────────┤
│  Containers: ps-php85, ps-mysql57, ps-redis72, ps-nginx127  │
│  Networks: php-stack-network                                 │
│  Volumes: data/, logs/                                       │
└─────────────────────────────────────────────────────────────┘
```

**⚠️ 注意**: `user_override_manager.rs` 已实现但未完全集成到配置生成流程中。

---

## 3. 核心工作流程

### 3.0 工作目录初始化流程 (Workspace Initialization)

**设计理念**：解耦软件本体与业务数据，实现跨平台无缝迁移。

```mermaid
sequenceDiagram
    participant U as 用户
    participant APP as php-stack.exe
    participant FS as 文件系统
    participant CFG as workspace.json

    U->>APP: 启动软件
    APP->>FS: 检查 exe 同级是否存在 workspace.json
    alt 文件存在
        APP->>CFG: 读取工作目录路径 (如 D:\demo)
        APP->>FS: 验证该目录下是否有 .env/docker-compose.yml
        alt 验证通过
            APP->>U: 进入主界面
        else 验证失败
            APP->>U: 提示“工作目录配置无效，请重新指定”
        end
    else 文件不存在
        APP->>U: 弹出“初始化向导”
        U->>APP: 选择或创建新目录作为工作区
        APP->>CFG: 写入 workspace.json { "workspace_path": "D:\demo" }
        APP->>FS: 在该目录下创建基础结构 (.env, www/, services/...)
        APP->>U: 进入主界面
    end
```

**配置文件格式 (`workspace.json`)**:
```json
{
  "workspace_path": "D:\\demo",
  "last_updated": "2026-04-21T10:00:00Z"
}
```

**备份与恢复逻辑**:
*   **备份**: 仅打包 `workspace.json` 中指定的目录内容。任何位于该目录之外的文件（如用户手动选择的系统级配置文件）均不予备份，并在 UI 层给予明确提示。
*   **恢复**: 在新环境（如 macOS）中，用户先指定一个新的工作目录路径，软件将 ZIP 包内的所有内容解压至该路径，并自动更新本地的 `workspace.json`。

---

### 3.1 环境配置与启动流程（主要流程）

```mermaid
sequenceDiagram
    participant U as 用户
    participant FE as 前端 (Vue)
    participant CMD as Commands
    participant CG as ConfigGenerator
    participant VM as VersionManifest
    participant UOM as UserOverrideManager
    participant FS as 文件系统
    participant DC as Docker Compose

    U->>FE: 选择服务版本 (MySQL 8.4)
    FE->>CMD: invoke('generate_env_config', config)
    CMD->>CG: generate_env(config)
    CG->>VM: new()
    VM->>VM: 加载 version_manifest.json
    VM-->>CG: 返回默认配置
    Note over CG,UOM: ⚠️ 当前未集成 UOM
    CG->>CG: 生成 .env 内容
    CG->>FS: 写入 .env 文件
    CG->>CG: 生成 docker-compose.yml
    CG->>FS: 写入 docker-compose.yml
    CG->>FS: 创建 services/ 目录
    CG-->>CMD: 返回成功
    CMD-->>FE: 返回结果
    FE->>U: 显示"应用成功"
    
    U->>FE: 点击"一键启动"
    FE->>FE: 显示确认弹窗
    U->>FE: 点击"直接启动"
    FE->>CMD: invoke('start_environment')
    CMD->>DC: docker compose down (清理旧容器)
    DC-->>CMD: 清理完成
    CMD->>CMD: ⏳ 等待容器完全停止...
    loop 循环检测（最多 10 次）
        CMD->>DM: list_ps_containers()
        DM->>DC: 获取 ps- 前缀容器列表
        DC-->>DM: 返回容器状态
        DM-->>CMD: 返回 PsContainer[]
        alt 所有 ps- 容器已停止
            CMD->>CMD: ✅ 所有 ps- 容器已完全停止
        else 仍有容器运行
            alt 第 10 次检测（超时）
                CMD->>CMD: ⚠️ 等待超时，显示未停止的容器
            else 继续等待
                CMD->>CMD: 等待 1 秒后再次检测
            end
        end
    end
    CMD->>CMD: 🔍 检查端口冲突...
    CMD->>DM: list_all_running_containers()
    DM->>DC: 获取所有运行中的容器
    DC-->>DM: 返回 Container[]
    DM-->>CMD: 返回容器列表
    CMD->>FS: load_existing_config()
    FS-->>CMD: 返回 EnvConfig
    CMD->>CMD: 遍历配置端口，检查冲突
    alt 检测到冲突
        CMD->>CMD: ❌ 返回 PORT_CONFLICT 错误
        CMD-->>FE: 错误信息
        FE->>FE: 显示 ConfirmDialog
        FE->>U: 显示冲突详情和解决方案
        U->>FE: 选择"忽略并继续"或"取消启动"
        alt 用户取消
            FE->>U: 中止启动
        else 用户继续
            FE->>CMD: 再次 invoke('start_environment')
            CMD->>DC: docker compose --progress plain up -d
        end
    else 无冲突
        CMD->>DC: docker compose --progress plain up -d
    end
    DC->>DC: 读取 .env 和 docker-compose.yml
    DC->>DC: 拉取镜像 (mysql:8.4)
    DC->>DC: 启动容器
    DC-->>CMD: 流式输出日志
    CMD-->>FE: 实时推送日志
    FE->>U: 显示启动状态
```

**端口冲突检测机制**：

#### 1. 检测时机
- **位置**: `start_environment` 命令中，清理旧容器后、启动新容器前
- **流程**: 
  ```
  清理旧容器 → 等待 ps- 容器停止 → 检查端口冲突 → 启动新容器
  ```

#### 2. 容器停止等待机制（循环检测）
- **目的**: 避免检测到正在停止的 ps- 容器，导致误判
- **实现**: 
  - 循环调用 `list_ps_containers()` 检查 ps- 前缀容器状态
  - 最多等待 10 次，每次间隔 1 秒
  - 所有 ps- 容器停止后立即继续，不浪费时间
  - 超时时显示未停止的容器列表，方便排查
- **优势**:
  - ✅ 自适应：不同机器、不同容器数量都能准确判断
  - ✅ 高效：容器停止后立即继续，不等固定时间
  - ✅ 可靠：基于 Docker API 实际状态，而非猜测

#### 3. 端口冲突检测方式
- **API**: `list_all_running_containers()` - 获取所有运行中的容器（包括外部容器）
- **冲突判断**: 遍历配置中的端口，检查是否被其他容器的 `ports` 数组占用
- **返回格式**: `PORT_CONFLICT:端口 X (服务A) 被容器 Y 占用; 端口 Z (服务B) 被容器 W 占用`

#### 4. 前端处理逻辑
- **错误捕获**: 检测 `errorMsg.startsWith('PORT_CONFLICT:')`
- **弹窗提示**: 使用 ConfirmDialog 显示冲突详情
- **用户选择**:
  - "取消启动": 终止流程，不执行 `docker compose up`
  - "忽略并继续": 再次调用 `start_environment`，跳过端口检查

#### 5. 整体优势
- ✅ **完全跨平台**: Docker API 统一接口，无需系统特定命令
- ✅ **精准定位**: 显示容器名、镜像、ID，而非进程名
- ✅ **无需权限**: 不需要 netstat/tasklist 等系统命令
- ✅ **用户友好**: 提供明确的解决命令和自定义弹窗
- ✅ **时序可靠**: 循环检测确保容器完全停止后再检查

### 3.2 版本映射查询流程

```mermaid
sequenceDiagram
    participant U as 用户
    participant FE as SoftwareSettings.vue
    participant CMD as Commands
    participant VM as VersionManifest

    U->>FE: 打开"软件设置"页面
    FE->>CMD: invoke('get_version_mappings')
    CMD->>VM: new()
    VM->>VM: 加载 version_manifest.json
    VM->>VM: 遍历所有服务版本
    VM-->>CMD: 返回 JSON 数据
    CMD-->>FE: 返回版本列表
    FE->>U: 展示表格 (应用名/版本/标签/状态)
    U->>FE: 点击"复制"按钮
    FE->>FE: 复制到剪贴板
    FE->>U: 显示"已复制"提示
```

### 3.3 备份流程

```mermaid
graph LR
    A[用户选择备份选项] --> B[BackupEngine.create_backup]
    B --> C[停止容器]
    C --> D[导出数据库 mysqldump]
    D --> E[打包配置文件]
    E --> F[生成 manifest.json]
    F --> G[计算 SHA256 校验和]
    G --> H[创建 ZIP 备份包]
    H --> I[发送进度事件]
    I --> J[完成]
```

### 3.4 环境恢复流程（分步卡片式交互）

**设计理念**：采用向导式分步卡片布局，用户主动控制节奏，避免信息过载。

```mermaid
sequenceDiagram
    participant U as 用户
    participant FE as RestorePage.vue
    participant CMD as Commands
    participant RE as RestoreEngine
    participant FS as 文件系统

    Note over U,FS: 步骤 1: 选择文件
    U->>FE: 点击“浏览文件”
    FE->>U: 打开文件选择对话框
    U->>FE: 选择 backup.zip
    FE->>FE: 标记步骤 1 完成
    FE->>FE: 自动进入步骤 2

    Note over U,FS: 步骤 2: 预览内容
    U->>FE: 点击“开始预览”
    FE->>CMD: invoke('preview_restore', zipPath)
    CMD->>RE: RestoreEngine::preview()
    RE->>FS: 读取 ZIP 中的 manifest.json
    RE->>RE: 解析备份清单
    RE->>RE: 检测端口冲突
    RE-->>CMD: 返回 RestorePreview
    CMD-->>FE: 返回预览数据
    FE->>FE: 显示备份摘要、服务列表
    FE->>FE: 标记步骤 2 完成
    FE->>U: 显示“下一步：校验”按钮
    
    U->>FE: 点击“下一步：校验 →”
    FE->>FE: 进入步骤 3

    Note over U,FS: 步骤 3: 校验完整性
    U->>FE: 点击“开始校验”
    FE->>CMD: invoke('verify_backup', zipPath)
    CMD->>RE: RestoreEngine::verify_integrity()
    RE->>FS: 读取 ZIP 中的所有文件
    RE->>RE: 计算每个文件的 SHA256
    RE->>RE: 与 manifest 中的哈希对比
    alt 校验通过
        RE-->>CMD: 返回 true
        CMD-->>FE: 返回 true
        FE->>FE: 显示“✓ SHA256 校验通过”
        FE->>FE: 标记步骤 3 完成
        FE->>U: 显示“下一步：开始恢复”按钮
    else 校验失败
        RE-->>CMD: 返回 false
        CMD-->>FE: 返回 false
        FE->>U: 显示“✗ 校验失败，备份可能已损坏”
    end

    U->>FE: 点击“下一步：开始恢复 →”
    FE->>FE: 进入步骤 4

    Note over U,FS: 步骤 4: 开始恢复
    U->>FE: 点击“确认并开始恢复”
    FE->>FE: 弹出 ConfirmDialog
    FE->>U: 显示恢复影响说明
    Note right of U: • 将覆盖 N 个配置文件<br/>• 服务配置详情<br/>• 端口映射调整<br/>• 备份时的警告<br/>• 注意：现有配置将被覆盖
    
    alt 用户取消
        U->>FE: 点击“取消”
        FE->>U: 中止恢复
    else 用户确认
        U->>FE: 点击“开始恢复”
        FE->>CMD: invoke('execute_restore', zipPath, portOverrides)
        CMD->>RE: RestoreEngine::restore()
        RE->>FS: 解压 .env 文件
        RE->>FS: 应用端口覆盖（如有）
        RE->>FS: 解压 docker-compose.yml
        RE->>FS: 解压 services/ 目录
        RE->>FS: 解压 vhosts/ 到 nginx/conf.d/
        RE->>FS: 解压 projects/ 到 SOURCE_DIR
        RE->>FS: 解压 database/ SQL 文件
        RE-->>CMD: 返回 RestoreResult
        CMD-->>FE: 返回成功
        FE->>FE: 标记步骤 4 完成
        FE->>U: 显示“✓ 环境恢复成功！”
        FE->>U: 提示“点击一键启动即可运行环境”
    end
```

**关键设计决策**：

#### 1. 分步卡片式布局
- **每个步骤独立卡片**：一次只显示当前步骤的内容
- **操作按钮固定在卡片底部**：无需滚动即可操作
- **平滑过渡动画**：300ms fade + slide 效果
- **视觉引导清晰**：蓝色“下一步”按钮引导用户继续

#### 2. 手动控制节奏
- **不自动跳转**：每步完成后停留在当前步骤
- **显示操作结果**：用户有足够时间查看信息
- **Toast 提示反馈**：告知用户当前状态
- **“下一步”按钮**：用户主动决定是否继续

#### 3. 职责分离原则
- **恢复 = 文件解压**：仅处理配置文件还原
- **启动 = 镜像拉取+容器运行**：在“一键启动”时处理
- **移除镜像检测**：避免用户在恢复阶段产生困惑
- **简化用户心智模型**：恢复和启动职责明确

#### 4. 端口冲突处理
- **预览时检测**：`detect_port_conflicts()` 检查端口占用
- **自动分配建议**：为每个冲突端口推荐可用端口
- **用户可修改**：提供输入框让用户自定义端口
- **恢复时应用**：`port_overrides` 参数应用到 .env 文件

#### 5. 完整性校验
- **SHA256 验证**：确保备份文件未被篡改
- **逐文件校验**：遍历 manifest 中的所有文件
- **即时反馈**：校验通过后才能进入下一步
- **安全保障**：防止损坏或恶意的备份包

**技术实现**：

```rust
// RestorePreview 结构体（精简版）
pub struct RestorePreview {
    pub manifest: BackupManifest,
    pub port_conflicts: Vec<PortConflict>,
    pub file_count: usize,
    // missing_images 字段已移除（职责分离）
}

// 恢复流程核心逻辑
pub async fn restore(
    zip_path: &str,
    project_root: &Path,
    port_overrides: HashMap<String, u16>,
    app_handle: Option<&tauri::AppHandle>,
) -> Result<RestoreResult, String> {
    // 1. 解析 manifest
    // 2. 解压 .env（应用端口覆盖）
    // 3. 解压 docker-compose.yml
    // 4. 解压 services/ 目录
    // 5. 解压 vhosts/ 到 nginx/conf.d/
    // 6. 解压 projects/ 到 SOURCE_DIR
    // 7. 解压 database/ SQL 文件
    // 8. 发送进度事件
}
```

**用户体验优化**：

| 方面 | 优化前 | 优化后 |
|------|--------|--------|
| **页面长度** | 很长，需要滚动 | 紧凑，一屏显示 |
| **操作按钮** | 在页面底部 | 固定在卡片底部 |
| **信息密度** | 高，容易 overwhelm | 低，一次专注一件事 |
| **视觉焦点** | 分散 | 集中在当前步骤 |
| **步骤切换** | 无动画 | 平滑过渡动画 |
| **用户控制** | 被动接受 | 主动决定 |
| **理解时间** | 不足 | 充足 |

**相关文件**：
- 前端：`src/components/RestorePage.vue`
- 后端：`src-tauri/src/engine/restore_engine.rs`
- 类型定义：`src/types/env-config.ts`

---

## 4. 模块详细说明

### 4.1 配置生成器 (ConfigGenerator)

**位置**: `src-tauri/src/engine/config_generator.rs`

**职责**:
- 根据用户选择生成 `.env` 文件
- 生成 `docker-compose.yml` 文件
- 创建 `services/` 目录结构并复制模板

**关键方法**:
```rust
pub fn generate_env(config: &EnvConfig) -> EnvFile
pub fn generate_compose(config: &EnvConfig) -> String
pub fn generate_service_dirs(config: &EnvConfig, root: &Path) -> Result<(), String>
```

**版本映射集成**:
```rust
// 当前实现
let manifest = VersionManifest::new();
let image_tag = manifest
    .get_image_info(&VmServiceType::Mysql, &service.version)
    .map(|info| info.tag.clone())
    .unwrap_or(service.version.clone());

env.set("MYSQL84_VERSION", &image_tag); // "8.4"
```

### 4.2 版本清单管理器 (VersionManifest)

**位置**: `src-tauri/src/engine/version_manifest.rs`  
**数据文件**: `src-tauri/services/version_manifest.json`

**职责**:
- 管理服务版本与 Docker 镜像标签的映射
- 提供版本验证和推荐功能
- 检测 EOL (End of Life) 版本

**数据结构**:
```json
{
  "mysql": {
    "8.4": {
      "image": "mysql",
      "tag": "8.4",
      "eol": false,
      "description": "MySQL 8.4 LTS (最新长期支持版)"
    }
  }
}
```

**关键方法**:
```rust
pub fn get_image_info(service_type, version) -> Option<&ImageInfo>
pub fn get_full_image_name(service_type, version) -> Option<String>
pub fn is_version_valid(service_type, version) -> bool
pub fn get_recommended_version(service_type) -> Option<&String>
```

### 4.3 用户覆盖管理器 (UserOverrideManager) ⚠️

**位置**: `src-tauri/src/engine/user_override_manager.rs`  
**配置文件**: `src-tauri/.user_version_overrides.json`

**状态**: ✅ 核心逻辑已实现，❌ 未集成到配置生成流程

**设计目标**:
- 允许用户自定义特定版本的 Docker 标签
- 优先级：用户配置 > 默认配置
- 支持保存/删除/重置操作

**合并策略**:
```rust
pub fn get_merged_image_info(&self, service_type, version) -> Option<ImageInfo> {
    // 1. 检查用户是否有覆盖配置
    if let Some(user_override) = self.user_overrides.get(...).and_then(...) {
        // 2. 获取默认配置作为基础
        if let Some(default_info) = self.default_manifest.get_image_info(...) {
            // 3. 返回合并后的配置（用户 tag 优先）
            return Some(ImageInfo {
                tag: user_override.tag.clone(), // ← 用户覆盖
                ..default_info.clone()
            });
        }
    }
    // 4. 没有用户覆盖，返回默认配置
    self.default_manifest.get_image_info(...)
}
```

**待完成工作**:
1. 在 `config_generator.rs` 中使用 `UserOverrideManager` 替代 `VersionManifest`
2. 添加后端 Command API (`save_user_override`, `remove_user_override`)
3. 前端添加编辑 UI

### 4.4 其他核心模块

| 模块 | 位置 | 职责 |
|------|------|------|
| **EnvParser** | `env_parser.rs` | .env 文件解析与格式化，保留注释 |
| **MirrorManager** | `mirror_manager.rs` | 统一镜像源管理（Docker/APT/Composer/NPM） |
| **BackupEngine** | `backup_engine.rs` | 环境备份（ZIP + manifest + SHA256） |
| **RestoreEngine** | `restore_engine.rs` | 环境恢复（验证 + 还原） |
| **DockerManager** | `docker/manager.rs` | 容器列表、启停操作 |
| **PortChecker** | `src/utils/portChecker.ts` | 前端端口冲突检测工具（基于 Docker API） |

---

## 5. 数据流图

### 5.1 配置文件生成数据流

```
用户输入 (GUI)
    ↓
EnvConfig { services: [...], source_dir, timezone }
    ↓
ConfigGenerator.generate_env()
    ↓
VersionManifest.new() → 加载 version_manifest.json
    ↓
[可选] UserOverrideManager.new() → 加载 .user_version_overrides.json
    ↓
合并配置 (用户覆盖 > 默认)
    ↓
EnvFile { entries: [(key, value), ...] }
    ↓
写入 .env 文件
    ↓
ConfigGenerator.generate_compose()
    ↓
写入 docker-compose.yml
    ↓
ConfigGenerator.generate_service_dirs()
    ↓
创建 services/{php85,mysql57,...}/ 目录
    ↓
复制模板文件 (Dockerfile, php.ini, mysql.cnf, ...)
```

### 5.2 版本映射数据流

```
version_manifest.json (静态数据)
    ↓
VersionManifest::new() (嵌入到二进制)
    ↓
get_image_info("mysql", "8.4")
    ↓
ImageInfo { image: "mysql", tag: "8.4", eol: false }
    ↓
full_name() → "mysql:8.4"
    ↓
写入 .env: MYSQL84_VERSION=8.4
    ↓
docker-compose.yml: image: mysql:${MYSQL84_VERSION}
    ↓
Docker Compose 解析 → 拉取 mysql:8.4
```

### 5.3 端口冲突检测数据流

```
用户点击"直接启动"
    ↓
load_existing_config() → 读取 .env 文件
    ↓
EnvConfig { services: [...] }
    ↓
extractPortsFromConfig() → 提取需要的端口
    ↓
Map { 3306: "mysql80", 6379: "redis82", ... }
    ↓
list_containers() → 获取运行中的容器
    ↓
Container[] { id, name, image, ports, state }
    ↓
遍历配置端口，检查是否有容器占用
    ↓
conflicts: ContainerPortConflict[]
    ↓
alt 有冲突
    formatContainerConflictMessage()
    ↓
显示冲突详情（容器名、镜像、ID）
    ↓
用户选择：继续 / 取消
    else 无冲突
    继续启动流程
end
```

### 5.4 ConfirmDialog 交互规范

**位置**: `src/components/ConfirmDialog.vue` + `src/composables/useConfirmDialog.ts`

**设计理念**: 关键确认操作必须通过明确的按钮选择，禁止点击外部关闭。

**使用场景**:
- 端口冲突检测（是否继续启动）
- 配置文件覆盖确认
- 删除/停止等危险操作
- 其他需要用户明确确认的场景

**交互流程**:
```mermaid
graph TD
    A[调用 showConfirm] --> B[显示 ConfirmDialog]
    B --> C{用户操作}
    C -->|点击遮罩层| D[无反应，弹窗保持]
    C -->|点击取消按钮| E[返回 false]
    C -->|点击确认按钮| F[返回 true]
    C -->|按 ESC 键| E
    D --> C
    E --> G[执行取消逻辑]
    F --> H[执行确认逻辑]
```

**API 示例**:
```typescript
import { showConfirm } from '../composables/useConfirmDialog';

const result = await showConfirm({
  title: '⚠️ 检测到端口冲突',
  message: `发现以下端口冲突：\n\n• 端口 80 (Nginx127) 被容器 nginx 占用\n\n是否继续启动？`,
  confirmText: '忽略并继续',
  cancelText: '取消启动',
  type: 'warning'  // 'danger' | 'warning' | 'info'
});

if (result) {
  // 用户点击了"忽略并继续"
} else {
  // 用户点击了"取消启动"或按 ESC
}
```

**优势**:
- ✅ **防止误触**: 点击外部不关闭，避免关键操作丢失
- ✅ **强制确认**: 用户必须明确选择，减少操作失误
- ✅ **统一体验**: 所有重要确认使用相同交互模式
- ✅ **可访问性**: 支持键盘操作（ESC 取消，Enter 确认）

---

## 6. 关键技术决策

### 6.1 为什么使用版本清单系统？

**问题**:
- 用户选择版本号 `8.4`，但 Docker Hub 上的标签可能是 `8.4`
- 不同服务的标签格式不一致（PHP: `8.4-fpm`, Redis: `7.2-alpine`）
- 新版本发布时需要多处修改代码

**解决方案**:
- 集中管理版本映射在 `version_manifest.json`
- 配置生成器自动查询正确的标签
- 添加新版本只需编辑 JSON 文件

**优势**:
- ✅ 解耦用户界面与 Docker 标签
- ✅ 单一数据源，易于维护
- ✅ 支持 EOL 检测和版本推荐

### 6.2 为什么采用用户覆盖机制？

**设计理念**:
- 默认配置由开发者维护（安全性）
- 高级用户可以自定义（灵活性）
- 平衡安全性和可控性

**实现策略**:
- 用户配置文件优先级高于默认配置
- 支持单个版本覆盖，不影响其他版本
- 可随时重置为默认配置

### 6.3 为什么使用 `${VAR}` 插值？

**docker-compose.yml 设计**:
```yaml
mysql84:
  image: mysql:${MYSQL84_VERSION}  # ← 使用变量
  ports:
    - "${MYSQL84_HOST_PORT}:3306"
```

**优势**:
- ✅ .env 和 docker-compose.yml 解耦
- ✅ 修改配置无需重新生成 compose 文件
- ✅ 符合 Docker Compose 最佳实践

### 6.4 为什么采用 Docker API 进行端口检测？

**问题**:
- 传统方法使用 `netstat`/`lsof` 检查宿主机端口
- 跨平台兼容性差（Windows/Linux/macOS 命令不同）
- 需要管理员权限
- 显示进程名而非容器名，用户难以理解

**解决方案**: 使用 Docker API (`list_all_running_containers`)

**优势**:
- ✅ **完全跨平台**: Docker API 统一接口
- ✅ **精准定位**: 显示容器名、镜像、ID
- ✅ **无需权限**: 不需要系统特定命令
- ✅ **简化实现**: 减少 90+ 行后端代码

### 6.5 为什么采用循环检测等待容器停止？

**问题**:
- 硬编码等待时间（如 `sleep(2)`）不可靠
- 可能等待时间不足（容器还在停止）
- 可能等待时间过长（浪费时间）
- 无法知道容器是否真的停止了

**解决方案**: 循环检测 ps- 容器状态

**实现**:
```rust
for attempt in 1..=10 {
    let ps_containers = manager.list_ps_containers().await?;
    let running = ps_containers.iter()
        .filter(|c| c.state.contains("running"))
        .collect();
    
    if running.is_empty() {
        break; // 所有容器已停止
    }
    tokio::time::sleep(Duration::from_secs(1)).await;
}
```

**优势**:
- ✅ **自适应**: 不同机器、不同容器数量都能准确判断
- ✅ **高效**: 容器停止后立即继续，不等固定时间
- ✅ **可靠**: 基于 Docker API 实际状态，而非猜测
- ✅ **友好**: 超时时显示未停止的容器列表

### 6.6 为什么禁止点击 ConfirmDialog 外部关闭？

**问题**:
- 点击遮罩层会触发 `handleCancel`
- 关键确认操作（如端口冲突）容易误触丢失
- 用户体验差，需要重新操作

**解决方案**: 移除 `@click.self` 事件处理器

**设计理念**:
- 关键确认必须通过明确的按钮选择
- 防止误触导致操作中断
- 强制用户认真阅读提示信息

**影响范围**:
- 端口冲突确认
- 配置文件覆盖确认
- 删除/停止等危险操作
- 所有重要确认场景

---

## 7. 扩展指南

### 7.1 添加新的服务版本

**步骤 1**: 编辑 `version_manifest.json`
```json
{
  "mysql": {
    "9.0": {
      "image": "mysql",
      "tag": "9.0-innovation",
      "eol": false,
      "description": "MySQL 9.0 Innovation (创新版)"
    }
  }
}
```

**步骤 2**: 创建服务模板目录
```bash
mkdir -p src-tauri/services/mysql90
cp src-tauri/services/mysql80/mysql.cnf src-tauri/services/mysql90/
```

**步骤 3**: 重新编译
```bash
cd src-tauri && cargo build
```

**无需修改任何 Rust 代码！**

### 7.2 添加用户自定义标签

**方法 1: 手动编辑**（当前可用）
```json
// src-tauri/.user_version_overrides.json
{
  "mysql": {
    "8.4": {
      "tag": "8.4-aliyun",
      "description": "使用阿里云镜像"
    }
  }
}
```

**方法 2: 通过 UI**（待实现）
1. 打开"软件设置"页面
2. 点击版本行的"编辑"按钮
3. 输入新的标签和描述
4. 点击"保存"

### 7.3 添加新的后端 Command

**步骤 1**: 在 `commands.rs` 中添加函数
```rust
#[tauri::command]
pub fn my_new_command(param: String) -> Result<String, String> {
    // 实现逻辑
    Ok(format!("Result: {}", param))
}
```

**步骤 2**: 在 `lib.rs` 中注册
```rust
.invoke_handler(tauri::generate_handler![
    // ... 其他命令
    commands::my_new_command,
])
```

**步骤 3**: 前端调用
```typescript
const result = await invoke('my_new_command', { param: 'test' });
```

---

## 📚 附录

### A. 文件结构概览

```
php-stack/
├── src/                          # 前端代码
│   ├── components/
│   │   ├── EnvConfigPage.vue
│   │   ├── MirrorPanel.vue
│   │   ├── BackupPage.vue
│   │   ├── RestorePage.vue
│   │   └── SoftwareSettings.vue
│   └── types/
│       └── version-mapping.ts
├── src-tauri/
│   ├── src/
│   │   ├── commands.rs           # API 命令
│   │   ├── lib.rs                # 插件注册
│   │   ├── engine/
│   │   │   ├── config_generator.rs
│   │   │   ├── version_manifest.rs
│   │   │   ├── user_override_manager.rs
│   │   │   └── ...
│   │   └── docker/
│   │       └── manager.rs
│   ├── services/
│   │   ├── version_manifest.json  # 版本清单
│   │   ├── php85/
│   │   ├── mysql57/
│   │   └── ...
│   └── .user_version_overrides.json  # 用户覆盖配置
├── .env                          # 生成的环境变量
├── docker-compose.yml            # 生成的 Compose 文件
└── services/                     # 运行时服务目录
    ├── php85/
    ├── mysql57/
    └── ...
```

### B. 常用命令

```bash
# 开发模式
npm run tauri dev

# 运行测试
cd src-tauri && cargo test

# 构建生产版本
npm run tauri build

# 查看版本映射
cat src-tauri/services/version_manifest.json | jq
```

### C. 相关文档

- [VERSION_MANIFEST.md](./src-tauri/docs/VERSION_MANIFEST.md) - 版本清单系统详细说明
- [VERSION_MANIFEST_VERIFICATION.md](./src-tauri/docs/VERSION_MANIFEST_VERIFICATION.md) - 验证报告
- [README.md](./README.md) - 项目总览

---

**文档维护**: 每次重大架构变更时，请同步更新此文档。
