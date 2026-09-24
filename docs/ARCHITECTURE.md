# PHP-Stack 系统架构文档

> **版本**: v0.3.1
> **最后更新**: 2026-09-24
> **配套文档**: [README.md](../README.md)（用户视角）、[DEV.md](../DEV.md)（开发者指南）、[AGENTS.md](../AGENTS.md)（AI Agent 协作规范）

---

## 目录

- [1. 项目概述](#1-项目概述)
- [2. 系统架构图](#2-系统架构图)
- [3. 模块概览](#3-模块概览)
- [4. 关键设计决策](#4-关键设计决策)
- [5. 核心流程](#5-核心流程)
- [6. 服务模板体系](#6-服务模板体系)
- [7. 日志系统](#7-日志系统)
- [8. 应用数据落点（单一事实来源）](#8-应用数据落点单一事实来源)
- [9. 相关文档](#9-相关文档)

---

## 1. 项目概述

PHP-Stack 是一个基于 **Tauri v2 + Docker** 的跨平台 PHP 开发环境可视化管理工具。

**核心价值**：
- 🎯 **可视化配置** — GUI 替代手动编辑 `.env` 和 `docker-compose.yml`
- 🌐 **镜像源加速** — 统一管理 Docker/APT/Composer/NPM 镜像源
- 💾 **环境备份恢复** — ZIP 打包 + SHA256 校验 + 恢复前自动回滚包
- 🔧 **多版本管理** — PHP/MySQL/Redis/Nginx 多版本共存，版本清单驱动
- 🌍 **国际化与主题** — 中/英双语、自动/明亮/暗黑三种模式
- 🐳 **权限映射** — PUID/PGID 用户映射，解决挂载目录权限问题

**技术栈**：前端 Vue 3 + TypeScript + Tailwind CSS v4；后端 Rust (Tauri v2) + bollard；容器 Docker + Compose。

## 2. 系统架构图

```
┌─────────────────────────────────────────────────────────────┐
│                        前端层 (Vue 3)                        │
├─────────────────────────────────────────────────────────────┤
│  App.vue (主框架: 侧边栏/日志面板/容器状态)                   │
│  ├── EnvConfigPage.vue     (环境配置 + 版本下拉)             │
│  ├── MirrorPanel.vue       (镜像源管理)                     │
│  ├── BackupPage.vue        (环境备份)                       │
│  ├── RestorePage.vue       (环境恢复向导)                   │
│  ├── SoftwareSettings.vue  (版本映射/用户覆盖)              │
│  ├── AboutPage.vue         (关于/日志等级/导出)             │
│  └── SettingsPage.vue / MigrationPage.vue / ...            │
│  api/ (命令封装: client/envConfig/mirror/backup/workspace/… )│
│  composables/ (useToast/useConfirmDialog/useDocker)          │
│  types/ (与 Rust 结构体对应的 TS 类型)                       │
└──────────────────────┬──────────────────────────────────────┘
                       │ Tauri IPC (invoke / event)
┌──────────────────────▼──────────────────────────────────────┐
│                      后端层 (Rust/Tauri)                     │
├─────────────────────────────────────────────────────────────┤
│  commands/ (API 入口，按业务域拆分)                          │
│  ├── docker.rs / env_config.rs / mirror.rs                   │
│  ├── backup.rs / workspace.rs / app.rs                       │
│  └── paths.rs (路径解析单一模块) / mod.rs                    │
├─────────────────────────────────────────────────────────────┤
│  engine/ (核心业务引擎)                                      │
│  ├── config_generator.rs     (配置生成 + 模板释放)           │
│  ├── version_manifest.rs     (版本清单 — VersionEntry)       │
│  ├── user_override_manager.rs(用户覆盖 — entry_kind)         │
│  ├── env_parser.rs           (.env 解析器)                   │
│  ├── mirror_manager.rs + mirror_config_manager.rs + mirror_config.rs(兼容层) │
│  ├── workspace_manager.rs    (工作目录管理)                  │
│  ├── backup_engine.rs / restore_engine.rs / backup_manifest.rs │
│  ├── config_extractor.rs     (运行时配置提取 Phase 3)        │
│  ├── site_manager.rs / user_config.rs / backup_options_store.rs │
├─────────────────────────────────────────────────────────────┤
│  docker/ (Docker 交互层: manager.rs / mirror.rs)             │
│  logging.rs (三层日志) / macros.rs (app_log!/ui_log!)        │
└──────────────────────┬──────────────────────────────────────┘
                       │ bollard / docker CLI
┌──────────────────────▼──────────────────────────────────────┐
│                     Docker Engine                            │
├─────────────────────────────────────────────────────────────┤
│  Containers: ps-php85, ps-mysql84, ps-redis82, ps-nginx128  │
│  Networks: php-stack-network                                 │
│  Volumes: data/, logs/                                       │
└─────────────────────────────────────────────────────────────┘
```

## 3. 模块概览

### 3.1 后端引擎（`src-tauri/src/engine/`）

| 模块 | 职责 |
|------|------|
| `config_generator.rs` | 根据 GUI 输入生成 `.env`、`docker-compose.yml`、释放 `services/` 模板（`resolve_template_dir` 回退 + `copy_template_file` 存在即跳过） |
| `version_manifest.rs` | 管理 `VersionEntry` 数据（`get_entry` / `get_available_entries` / `find_entry_by_env_prefix`），清单 `include_str!` 编译进二进制，可被 app_data_dir 外部清单覆盖 |
| `user_override_manager.rs` | 管理 `.user-config/version_overrides.json`（`entry_kind: override/custom`），`get_merged_entry` 合并用户镜像 tag |
| `env_parser.rs` | `.env` 文件可靠读写，保留注释和空行，往返一致 |
| `mirror_manager.rs` / `mirror_config_manager.rs` | 镜像源预设与用户配置管理；`mirror_config.rs` 为向后兼容层 |
| `workspace_manager.rs` | 管理 `workspace.json`（app_data_dir），解耦软件本体与业务数据 |
| `backup_engine.rs` / `backup_manifest.rs` | ZIP 备份（manifest + SHA256 + 64KB 分块流式写入） |
| `restore_engine.rs` | 备份验证与还原：zip-slip 防护、预览端口冲突检测、恢复前自动回滚包、返回完整明细 |
| `config_extractor.rs` | 运行时按需配置提取（Phase 3）：`docker create + cp` 从官方镜像提取默认配置 |
| `site_manager.rs` / `user_config.rs` / `backup_options_store.rs` | 站点定义 / 用户配置收纳 / 备份选项持久化 |

### 3.2 前端组件（`src/components/`）

| 组件 | 职责 |
|------|------|
| `EnvConfigPage.vue` | 服务版本选择、端口配置、PHP 扩展（多服务独立输入）、应用配置 |
| `SoftwareSettings.vue` | 版本映射表格、用户 Override 编辑（override/custom） |
| `MirrorPanel.vue` | 镜像源预设、独立配置、连接测试 |
| `BackupPage.vue` / `RestorePage.vue` | 备份选项与分步恢复向导（预览→校验→确认→明细结果） |
| `AboutPage.vue` / `SettingsPage.vue` | 关于/日志等级/导出；应用设置 |
| `CustomSelect.vue` / `UiTabs.vue` / `ConfirmDialog.vue` / `Toast.vue` | 通用 UI 组件 |
| `VersionHelpModal.vue` / `ImagePullConfirmModal.vue` | 版本帮助弹窗 / 镜像拉取确认弹窗（Phase 3 配套） |
| `WorkspaceInitDialog.vue` / `WorkspaceMissingDialog.vue` | 工作区初始化与缺失提示 |

### 3.3 核心数据结构

**`VersionEntry`**（Rust — `version_manifest.rs`）：

```rust
pub struct VersionEntry {
    pub display_name: String,   // "PHP 8.2"
    pub image_tag: String,      // "php:8.2-fpm"（完整镜像名）
    pub service_dir: String,    // "php82"（配置目录名）
    pub default_port: u16,      // 9000
    pub show_port: bool,        // PHP 不显示端口配置
    pub eol: bool,
    pub description: Option<String>,
}
```

**设计原则**：manifest 每条记录自描述。ID 即目录名（`php82`），`env_prefix` 由 `service_dir.to_uppercase()` 推导（`PHP82`），无需运行时格式转换。

## 4. 关键设计决策

### 4.1 版本清单系统（扁平化）

版本映射集中在 `version_manifest.json`，配置生成器直接使用 `entry.image_tag`，消除全部格式转换逻辑（`version.replace('.',"")` / `format!("{}:{}",image,tag)` / 前缀硬编码链）。添加新版本只需编辑 JSON（或由 `sync-version-manifest` 同步），无需修改 Rust 代码。

### 4.2 用户覆盖机制

默认配置由开发者维护（安全性），高级用户可自定义（灵活性）。`.user-config/version_overrides.json` 按 manifest ID 覆盖，`entry_kind` 必填：

| `entry_kind` | 语义 |
|---|---|
| `override` | 叠在清单基线上，只换 `image_tag`（+可选 `description`） |
| `custom` | 完整 VersionEntry 独立条目，可出现在环境配置下拉 |

优先级：用户覆盖 → 内置清单 → Dockerfile 默认值（兜底）。

### 4.3 docker-compose 变量插值

使用 `${VAR}` 插值语法，`.env` 与 `docker-compose.yml` 解耦，修改配置无需重新生成 compose 文件。

### 4.4 配置备份采用 ZIP 打包

`zip::ZipWriter`（Deflated）单一 ZIP 文件，含 `manifest.json` + SHA256 逐文件校验；失败即删除 ZIP，单文件失败不影响其他（容错优于严格）。备份写入采用 64KB 分块流式，避免大文件整体读入内存。

### 4.5 动态基础镜像 → Dockerfile 参数化

`build.args.PHP_BASE_IMAGE` 注入 `${PHP82_VERSION}` → Dockerfile `ARG PHP_BASE_IMAGE; FROM ${PHP_BASE_IMAGE}`。PHP Dockerfile 已收敛为参数化模板（`scripts/sync-php-dockerfile.mjs` 同步），新增版本不必复制目录。

### 4.6 容器状态枚举契约

前后端共享 `ContainerState` 枚举（`running/exited/...`），替换早期 `format!("{:?}")` 字符串 + `includes('running')` 猜测的脆弱做法。

### 4.7 恢复安全与回滚

- **zip-slip 防护**：解压前校验条目路径不得逃逸目标目录（拒绝 `..`、绝对路径、盘符）。
- **恢复前自动回滚包**：`execute_restore` 开始前把将被覆盖的文件打包为 `.restore_rollback_<ts>.zip`，出错可回退。
- **恢复结果明细**：始终返回 `RestoreResult`（恢复文件列表 + 逐条错误 + 回滚包路径），前端渲染结果卡片。
- **端口冲突**：预览时检测宿主机占用并给出建议端口；恢复仍写入原配置，不自动改写（用户启动前可手动调整）。

## 5. 核心流程

### 5.1 配置生成

```
用户输入 (GUI) → EnvConfig { services: [{ version: "php82", ... }] }
    → ConfigGenerator.generate_env(config, workspace_root)
    → VersionManifest.get_entry("php", "php82")
    → UserOverrideManager.get_merged_entry(...)   // 合并用户 image_tag
    → env_prefix = service_dir.to_uppercase()      // "PHP82"
    → env.set("PHP82_VERSION", entry.image_tag)
    → 写入 .env → 写入 docker-compose.yml → 释放 services/<dir>/ 模板（已存在则跳过）
```

### 5.2 环境启动

```
start_environment
    → docker compose down --remove-orphans（清理旧容器）
    → 等待 ps- 容器完全停止（循环检测，最多 10 次 × 1s）
    → 检查端口冲突（PORT_CONFLICT → 前端 ConfirmDialog：忽略并继续 / 取消）
    → docker compose up -d（后台启动）
    → 智能等待容器就绪（每 2s 检查 running，无硬超时；logs -f 进程异常退出视为失败）
```

> 启动流程的同步阻塞调用（`compose down` / `up` 等待 / 日志轮询）统一走 `spawn_blocking`。

### 5.3 备份与恢复

**备份**：选择选项（数据库/项目文件/日志等）→ `BackupEngine.create_backup` → 打包 `.env`、`docker-compose.yml`、`services/`、`.user-config/` 配置 → 生成 `manifest.json` → SHA256 校验 → 流式写入 ZIP → 进度事件。

**恢复**（分步向导）：
1. 选择备份文件 → 2. 预览（manifest 解析 + 端口冲突检测）→ 3. SHA256 完整性校验 → 4. 确认恢复（自动生成回滚包 → 解压 .env / compose / services / 其他 → 返回明细结果）。

## 6. 服务模板体系

### 6.1 三层结构

`src-tauri/services/` 下文件是三类性质不同的东西，上游可得性差异极大：

| 层 | 内容 | 策略 |
|---|---|---|
| L1 版本元数据 | `version_manifest.json` | **全自动同步**（official-images + endoflife.date） |
| L2 配置基线 | `php.ini` / `mysql.cnf` / `redis.conf` / `nginx.conf` | **运行时按需提取**（Phase 3） |
| L3 构建定义 | `Dockerfile` | **参数化模板手工维护**（项目自研，含 PUID/PGID/镜像源注入） |

**核心决策：同步放 CI，不放进 App。** App 保持离线可用，模板与版本清单作为随版本发布的快照内置；镜像源等"执行期联网"能力与"定义期联网"（运行时抓取上游定义）严格区分，后者不做。

### 6.2 `{name}{version}` 命名约定（一等约束）

`services/` 目录名（`php85`、`mysql84`...）是贯穿全项目的隐式契约，被以下位置消费：

| 位置 | 用法 |
|---|---|
| `version_manifest.json` → `service_dir` | 清单条目指向模板目录 |
| `config_generator` | `env_prefix = service_dir.to_uppercase()` |
| compose 服务名 / 容器名 | `ps-{service_dir}` |
| 目录映射 | `services/`、`data/`、`logs/` 三处同名 |
| `resolve_template_dir` | 按 `service_dir` 查找模板，找不到回退默认目录 |

**该约定是"项目停止维护后用户仍可自救"的根基**：用户加一条清单 + 建一个同名目录即可，无需改代码。任何改动不得破坏它。

**`service_dir` 与 ID 是两个独立字段**：新增版本可复用已有目录（`service_dir` 指向旧目录），配合 `resolve_template_dir` 回退，"加版本"退化为"改一行 JSON"。

### 6.3 Phase 3：运行时按需配置提取（`config_extractor.rs`）

集成在 `ConfigGenerator::apply → generate_service_dirs` 复制逻辑之前：

1. 工作区配置已存在（用户改过）→ 跳过，不覆盖
2. 内置模板有对应目录 → 直接复制（零成本）
3. 都没有 → 检查本地镜像 → 有则 `docker create + cp + rm`（**不删镜像**，供后续 `up` 复用）；无则汇总弹窗让用户确认后串行 pull
4. 提取失败 → 降级到 fallback 模板 + warning 日志，**不阻断 apply**

配置映射（硬编码在 Rust，跨版本稳定）：PHP `/usr/local/etc/php/php.ini-production`（5.x 无 `-production` 后缀）；MySQL `/etc/my.cnf`（5.x 为 `/etc/mysql/my.cnf`）；Redis `/usr/local/etc/redis/redis.conf`；Nginx `/etc/nginx/nginx.conf`。

### 6.4 红线

1. 永不覆盖用户已修改的文件（`copy_template_file` 目标存在则跳过）
2. 同步产物不自动合入（PR + 人工 review，防 rc/beta 推给用户）
3. 不引入 Docker Hub API 依赖（实测不通且限流）
4. Dockerfile 不从上游复制；不删除用户手写的清单条目

## 7. 日志系统

三层日志架构：

| 层级 | 目标 | 用途 |
|------|------|------|
| 文件日志 | `php-stack.log`（app_data_dir） | 持久化，启动时轮转保留最近 3 份 |
| 控制台 | 终端 | 开发调试（tracing + EnvFilter，可用 `RUST_LOG` 覆盖） |
| UI 日志 | 前端面板 | `env-log` 推送 `{ level, message }`，最多 200 条，按等级过滤着色 |

- `app_log!(level, module, ...)`：写文件 + 控制台，不推 UI
- `ui_log!(app, level, module, ...)`：先 `app_log!` 再 emit `env-log`；用户可见文案统一英文短句、无 emoji
- 前端 `useToast.ts` 管理日志缓冲与等级过滤（`php-stack-log-level`）；面板支持复制（当前可见条目）/清空/导出（文件日志）

## 8. 应用数据落点（单一事实来源）

> 路径以运行时解析为准；`app_data_dir` 为 Tauri 官方应用数据目录（Windows `%APPDATA%\com.php-stack.dev` 等）。

| 写入物 | 落点 | 负责模块 |
|--------|------|----------|
| `.env` / `docker-compose.yml` | **工作区**（`workspace.json` 配置路径） | `config_generator` / `env_config` |
| `services/` / `data/` / `logs/` | 工作区 | `config_generator` / Docker 挂载 |
| `.user-config/mirror_config.json` / `version_overrides.json` / `sites.json` | 工作区 | `mirror_config_manager` / `user_override_manager` / `site_manager` |
| 备份 ZIP / `.restore_rollback_*.zip` | 用户选择路径 / 工作区 | `backup_engine` / `commands::backup` |
| `workspace.json` | **app_data_dir** | `workspace_manager` |
| `php-stack.log` | **app_data_dir** | `logging`（轮转） |
| `services/version_manifest.json`（可选） | **app_data_dir** | `version_manifest`（外部清单覆盖；缺失用二进制内置） |

> 路径解析统一收口在 `commands/paths.rs`，禁止业务代码自行爬目录。

## 9. 相关文档

| 文档 | 用途 |
|------|------|
| [README.md](../README.md) | 项目介绍、用户快速开始 |
| [DEV.md](../DEV.md) | 开发者贡献指南：环境准备、开发/测试命令、扩展指南、版本同步工作流 |
| [AGENTS.md](../AGENTS.md) | AI Agent 协作规范 |

> 文档维护原则：架构级变更更新本文档；面向用户的变更更新 README；变更记录进 CHANGELOG；过时内容直接删除（git 历史可追溯），**不保留历史归档目录**。
