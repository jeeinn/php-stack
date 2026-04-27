# PHP-Stack 核心工作流程

> **版本**: v0.2.0 (2026-04-27)  
> ↩ [返回主架构文档](./ARCHITECTURE.md)

---

## 📋 目录

- [3.0 工作目录初始化流程](#30-工作目录初始化流程)
- [3.1 环境配置与启动流程](#31-环境配置与启动流程)
- [3.2 版本映射查询流程](#32-版本映射查询流程)
- [3.3 备份流程](#33-备份流程)
- [3.4 环境恢复流程](#34-环境恢复流程)

---

## 3.0 工作目录初始化流程

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
            APP->>U: 提示"工作目录配置无效，请重新指定"
        end
    else 文件不存在
        APP->>U: 弹出"初始化向导"
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
- **备份**: 仅打包 `workspace.json` 中指定的目录内容。位于该目录之外的文件不予备份，并在 UI 层给予明确提示。
- **恢复**: 在新环境中，用户先指定一个新的工作目录路径，软件将 ZIP 包内的所有内容解压至该路径，并自动更新本地的 `workspace.json`。

---

## 3.1 环境配置与启动流程

### 3.1.1 配置应用与备份机制

**设计理念**：采用 ZIP 打包方式备份配置文件，确保数据安全且易于管理。

```mermaid
sequenceDiagram
    participant U as 用户
    participant FE as 前端 (Vue)
    participant CMD as Commands
    participant CG as ConfigGenerator
    participant FS as 文件系统
    participant ZIP as ZIP Writer

    U->>FE: 选择服务版本并点击"应用配置"
    FE->>FE: 检查配置文件是否存在
    alt 文件存在
        FE->>U: 显示确认对话框（是否备份）
        U->>FE: 勾选"备份现有配置"
    end
    FE->>CMD: invoke('apply_env_config', config, enableBackup=true)
    CMD->>CG: ConfigGenerator::apply(config, project_root, enable_backup=true)
    
    Note over CG,ZIP: 阶段 1: 预检查备份
    CG->>FS: 检查 .env, docker-compose.yml, services/ 是否存在
    FS-->>CG: 返回存在的文件列表
    CG->>CG: 生成备份文件名: config_backup_YYYYMMDD_HHMMSS.zip
    
    Note over CG,ZIP: 阶段 2: 执行 ZIP 打包备份
    CG->>ZIP: 创建 ZIP 文件（Deflated 压缩）
    loop 遍历配置项
        CG->>FS: 读取文件/目录内容
        CG->>ZIP: 写入 ZIP
    end
    CG->>ZIP: finish()
    
    Note over CG,FS: 阶段 3: 生成新配置（始终生成全新 .env）
    CMD->>CG: generate_env(config, project_root)
    CG->>FS: 写入 .env 文件
    CG->>CG: generate_compose(config)
    CG->>FS: 写入 docker-compose.yml
    CG->>FS: 创建 services/ 目录结构
    CG-->>CMD: Ok(backed_up_files)
    CMD-->>FE: 返回备份文件列表
    FE->>U: 显示"配置应用成功 + 备份信息"
```

**备份文件格式**：
- **命名规则**: `config_backup_YYYYMMDD_HHMMSS.zip`
- **包含内容**: `.env`、`docker-compose.yml`、`services/`、`.user_mirror_config.json`、`.user_version_overrides.json`

**v0.2.0 变更**：
- `apply()` 始终生成全新 `.env`，不再读取现有 `.env` 进行合并
- `generate_env()` 接受 `project_root` 参数，不再使用内部 `get_project_root()`

### 3.1.2 完整启动流程

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

    U->>FE: 选择服务版本 (如 mysql84)
    FE->>CMD: invoke('generate_env_config', config)
    CMD->>CG: generate_env(config, project_root)
    CG->>VM: new()
    VM->>VM: 加载 version_manifest.json
    CG->>UOM: new(project_root)
    UOM->>UOM: 加载 .user_version_overrides.json
    CG->>UOM: get_merged_entry(&ServiceType::Mysql, "mysql84")
    UOM-->>CG: VersionEntry { image_tag: "mysql:8.4", service_dir: "mysql84", ... }
    CG->>CG: env_prefix = "MYSQL84" (service_dir.to_uppercase())
    CG->>CG: env.set("MYSQL84_VERSION", entry.image_tag)
    CG->>FS: 写入 .env 文件
    CG->>CG: generate_compose(config)
    CG->>FS: 写入 docker-compose.yml
    CG->>FS: 创建 services/ 目录（使用 resolve_template_dir()）
    CG-->>CMD: 返回成功
    CMD-->>FE: 返回结果
    FE->>U: 显示"应用成功"
    
    U->>FE: 点击"一键启动"
    FE->>CMD: invoke('start_environment')
    CMD->>DC: docker compose down (清理旧容器)
    DC-->>CMD: 清理完成
    CMD->>CMD: ⏳ 等待 ps- 容器完全停止（循环检测）
    CMD->>CMD: 🔍 检查端口冲突
    alt 检测到冲突
        CMD-->>FE: PORT_CONFLICT 错误
        FE->>U: 显示 ConfirmDialog（冲突详情）
        U->>FE: 选择"忽略并继续"或"取消启动"
    else 无冲突
        CMD->>DC: docker compose --progress plain up -d
        DC-->>CMD: 后台启动完成
        CMD->>CMD: 智能等待容器就绪（详见 LOGGING.md）
        CMD-->>FE: 返回成功
        FE->>U: 显示"环境启动成功"
    end
```

**v0.2.0 关键变更**：
- `ServiceEntry.version` 语义变更为 manifest ID（如 `"mysql84"`），不再是版本号
- 配置生成器直接使用 `entry.image_tag`，不再需要 `format!("{}:{}", image, tag)` 拼接
- 目录名直接使用 `entry.service_dir`，不再需要 `version.replace('.', "")` 计算
- 模板选择使用 `resolve_template_dir()` 辅助函数，消除 `if version.starts_with(...)` 硬编码链
- `UserOverrideManager` 已完全集成到配置生成流程

### 3.1.3 端口冲突检测机制

**检测时机**: `start_environment` 命令中，清理旧容器后、启动新容器前。

**流程**:
```
清理旧容器 → 等待 ps- 容器停止（循环检测，最多 10 次） → 检查端口冲突 → 启动新容器
```

**容器停止等待机制**:
- 循环调用 `list_ps_containers()` 检查 ps- 前缀容器状态
- 最多等待 10 次，每次间隔 1 秒
- 所有 ps- 容器停止后立即继续
- 超时时显示未停止的容器列表

**端口冲突检测**:
- 使用 `list_all_running_containers()` 获取所有运行中容器（包括外部容器）
- 遍历配置端口，检查是否被其他容器占用
- 返回格式: `PORT_CONFLICT:端口 X (服务A) 被容器 Y 占用`

**前端处理**:
- 检测 `errorMsg.startsWith('PORT_CONFLICT:')`
- 使用 ConfirmDialog 显示冲突详情
- 用户可选择"忽略并继续"或"取消启动"

---

## 3.2 版本映射查询流程

```mermaid
sequenceDiagram
    participant U as 用户
    participant FE as SoftwareSettings.vue
    participant CMD as Commands
    participant VM as VersionManifest
    participant UOM as UserOverrideManager

    U->>FE: 打开"软件设置"页面
    FE->>CMD: invoke('get_version_mappings')
    CMD->>VM: new()
    VM->>VM: 加载 version_manifest.json
    CMD->>UOM: new(project_root)
    UOM->>UOM: 加载 .user_version_overrides.json
    CMD->>VM: get_available_entries(service_type)
    VM-->>CMD: Vec<(&String, &VersionEntry)>（按版本号降序）
    CMD->>CMD: 合并 has_user_override 标记
    CMD-->>FE: 返回版本列表
    FE->>U: 展示表格 (display_name / image_tag / service_dir / 状态)
```

**返回数据结构（v0.2.0）**:
```json
{
  "php": [
    {
      "id": "php85",
      "display_name": "PHP 8.5",
      "image_tag": "php:8.5-fpm",
      "service_dir": "php85",
      "default_port": 9000,
      "show_port": false,
      "eol": false,
      "description": "PHP 8.5 (最新开发版)",
      "has_user_override": false
    }
  ]
}
```

**前端下拉列表**:
- `value` = `v.id`（如 `"php82"`）
- 显示文本 = `v.display_name → v.image_tag`（如 `"PHP 8.2 → php:8.2-fpm"`）
- 端口输入框根据 `v.show_port` 控制显示（PHP 不显示端口配置）

**用户 Override 操作**:
- `save_user_override(service_type, id, image_tag)` — 参数使用 manifest ID 和完整镜像名
- `remove_user_override(service_type, id)` — 参数使用 manifest ID

---

## 3.3 备份流程

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

**备份包内容**:
- `manifest.json` — 备份元数据（版本、时间戳、服务列表、文件校验和）
- `.env` — 环境变量配置
- `docker-compose.yml` — Compose 配置
- `services/` — 服务配置目录
- `.user_mirror_config.json` — 用户镜像源配置（如存在）
- `.user_version_overrides.json` — 用户版本覆盖配置（如存在）
- `database/` — 数据库导出（可选）
- `projects/` — 项目文件（可选，glob 模式匹配）

---

## 3.4 环境恢复流程

**设计理念**：采用向导式分步卡片布局，用户主动控制节奏，避免信息过载。

```mermaid
sequenceDiagram
    participant U as 用户
    participant FE as RestorePage.vue
    participant CMD as Commands
    participant RE as RestoreEngine
    participant FS as 文件系统

    Note over U,FS: 步骤 1: 选择文件
    U->>FE: 点击"浏览文件"
    FE->>U: 打开文件选择对话框
    U->>FE: 选择 backup.zip
    FE->>FE: 标记步骤 1 完成，自动进入步骤 2

    Note over U,FS: 步骤 2: 预览内容
    U->>FE: 点击"开始预览"
    FE->>CMD: invoke('preview_restore', zipPath)
    CMD->>RE: RestoreEngine::preview()
    RE->>FS: 读取 ZIP 中的 manifest.json
    RE->>RE: 解析备份清单 + 检测端口冲突
    RE-->>CMD: 返回 RestorePreview
    CMD-->>FE: 返回预览数据
    FE->>U: 显示备份摘要、服务列表

    Note over U,FS: 步骤 3: 校验完整性
    U->>FE: 点击"开始校验"
    FE->>CMD: invoke('verify_backup', zipPath)
    CMD->>RE: RestoreEngine::verify_integrity()
    RE->>RE: 计算每个文件的 SHA256 并与 manifest 对比
    alt 校验通过
        RE-->>FE: 显示"✓ SHA256 校验通过"
    else 校验失败
        RE-->>FE: 显示"✗ 校验失败，备份可能已损坏"
    end

    Note over U,FS: 步骤 4: 开始恢复
    U->>FE: 点击"确认并开始恢复"
    FE->>FE: 弹出 ConfirmDialog（恢复影响说明）
    U->>FE: 点击"开始恢复"
    FE->>CMD: invoke('execute_restore', zipPath, portOverrides)
    CMD->>RE: RestoreEngine::restore()
    RE->>FS: 解压 .env（应用端口覆盖）
    RE->>FS: 解压 docker-compose.yml
    RE->>FS: 解压 services/ 目录
    RE->>FS: 解压其他文件（vhosts、projects、database）
    RE-->>FE: 返回成功
    FE->>U: 显示"✓ 环境恢复成功！"
```

**关键设计决策**:

| 设计点 | 说明 |
|--------|------|
| **分步卡片** | 一次只显示当前步骤，减少信息过载 |
| **手动控制** | 不自动跳转，用户主动决定是否继续 |
| **职责分离** | 恢复 = 文件解压；启动 = 镜像拉取 + 容器运行 |
| **端口冲突** | 预览时检测，自动推荐可用端口，用户可修改 |
| **完整性校验** | SHA256 逐文件验证，校验通过才能进入下一步 |

**相关文件**:
- 前端: `src/components/RestorePage.vue`
- 后端: `src-tauri/src/engine/restore_engine.rs`
- 类型: `src/types/env-config.ts`

---

↩ [返回主架构文档](./ARCHITECTURE.md)
