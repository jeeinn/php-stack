# PHP-Stack 系统架构文档

> **版本**: v0.2.0 (2026-04-27)  
> **最后更新**: 2026-04-27  
> **维护者**: PHP-Stack Team

---

## 📋 目录

- [1. 项目概述](#1-项目概述)
- [2. 系统架构图](#2-系统架构图)
- [3. 模块概览](#3-模块概览)
- [4. 数据流概览](#4-数据流概览)
- [5. 文件结构](#5-文件结构)
- [6. 子文档索引](#6-子文档索引)

---

## 1. 项目概述

### 1.1 项目定位

PHP-Stack 是一个基于 **Tauri v2 + Docker** 的跨平台 PHP 开发环境可视化管理工具。

**核心价值**：
- 🎯 **可视化配置** — 替代手动编辑 `.env` 和 `docker-compose.yml`
- 🚀 **镜像源加速** — 统一管理 Docker/APT/Composer/NPM 镜像源
- 💾 **环境备份恢复** — ZIP 格式打包，支持 SHA256 完整性校验
- 🔧 **多版本管理** — 支持 PHP/MySQL/Redis/Nginx 多版本共存
- 🔄 **动态基础镜像** — 用户可自由切换 Debian/Alpine 或自定义镜像标签

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
│  ├── MirrorPanel.vue        (镜像源管理)                     │
│  ├── BackupPage.vue         (环境备份)                       │
│  ├── RestorePage.vue        (环境恢复)                       │
│  └── SoftwareSettings.vue   (软件设置/版本管理)              │
├─────────────────────────────────────────────────────────────┤
│  src/types/env-config.ts    (统一类型定义)                    │
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
│  └── get_version_mappings / save_user_override              │
├─────────────────────────────────────────────────────────────┤
│  engine/ (核心业务引擎)                                      │
│  ├── config_generator.rs       (配置生成器 + 动态镜像切换)   │
│  ├── version_manifest.rs       (版本清单 — VersionEntry)     │
│  ├── user_override_manager.rs  (用户覆盖管理器)              │
│  ├── env_parser.rs             (.env 解析器)                │
│  ├── mirror_manager.rs         (镜像源管理器)               │
│  ├── mirror_config_manager.rs  (镜像源配置管理)             │
│  ├── workspace_manager.rs      (工作目录管理)               │
│  ├── backup_engine.rs          (备份引擎)                   │
│  ├── restore_engine.rs         (恢复引擎)                   │
│  └── backup_manifest.rs        (备份清单模型)               │
├─────────────────────────────────────────────────────────────┤
│  docker/ (Docker 交互层)                                     │
│  ├── manager.rs                (容器管理)                    │
│  └── mirror.rs                 (镜像源切换)                  │
└──────────────────────┬──────────────────────────────────────┘
                       │ bollard (Docker SDK)
┌──────────────────────▼──────────────────────────────────────┐
│                     Docker Engine                            │
├─────────────────────────────────────────────────────────────┤
│  Containers: ps-php85, ps-mysql84, ps-redis82, ps-nginx128  │
│  Networks: php-stack-network                                 │
│  Volumes: data/, logs/                                       │
└─────────────────────────────────────────────────────────────┘
```

---

## 3. 模块概览

### 3.1 核心引擎模块

| 模块 | 文件 | 职责 |
|------|------|------|
| **版本清单** | `version_manifest.rs` | 管理 `VersionEntry` 数据，提供 `get_entry()`、`get_available_entries()`、`find_entry_by_env_prefix()` 等查询 API |
| **用户覆盖** | `user_override_manager.rs` | 管理 `.user_version_overrides.json`，通过 `get_merged_entry()` 合并用户自定义 `image_tag` |
| **配置生成** | `config_generator.rs` | 根据 GUI 输入生成 `.env`、`docker-compose.yml`、`services/` 目录 |
| **Env 解析** | `env_parser.rs` | `.env` 文件可靠读写，保留注释和空行 |
| **镜像源管理** | `mirror_manager.rs` | 统一管理 Docker/APT/Composer/NPM 镜像源 |
| **工作目录** | `workspace_manager.rs` | 管理 `workspace.json`，解耦软件本体与业务数据 |
| **备份引擎** | `backup_engine.rs` | ZIP 格式备份，含 manifest + SHA256 校验 |
| **恢复引擎** | `restore_engine.rs` | 备份包验证与环境还原 |

### 3.2 前端组件

| 组件 | 职责 |
|------|------|
| `EnvConfigPage.vue` | 服务版本选择、端口配置、PHP 扩展选择、一键生成配置 |
| `SoftwareSettings.vue` | 版本映射表格、用户 Override 编辑弹窗 |
| `MirrorPanel.vue` | 镜像源预设方案、独立配置、连接测试 |
| `BackupPage.vue` | 备份选项配置、进度显示 |
| `RestorePage.vue` | 分步卡片式恢复向导 |

### 3.3 核心数据结构（v0.2.0）

**`VersionEntry`**（Rust — `version_manifest.rs`）：
```rust
pub struct VersionEntry {
    pub display_name: String,   // "PHP 8.2"
    pub image_tag: String,      // "php:8.2-fpm"（完整镜像名）
    pub service_dir: String,    // "php82"（配置目录名）
    pub default_port: u16,      // 9000
    pub show_port: bool,        // false（PHP 不显示端口配置）
    pub eol: bool,              // false
    pub description: Option<String>,
}
```

**`VersionInfo`**（TypeScript — `env-config.ts`）：
```typescript
export interface VersionInfo {
  id: string;              // manifest ID，如 "php82"
  display_name: string;    // "PHP 8.2"
  image_tag: string;       // "php:8.2-fpm"
  service_dir: string;     // "php82"
  default_port: number;    // 9000
  show_port: boolean;      // false
  eol: boolean;
  description?: string;
  has_user_override?: boolean;
}
```

**`version_manifest.json` 结构**：
```json
{
  "php": {
    "php82": {
      "display_name": "PHP 8.2",
      "image_tag": "php:8.2-fpm",
      "service_dir": "php82",
      "default_port": 9000,
      "show_port": false,
      "eol": false,
      "description": "PHP 8.2 (活跃支持)"
    }
  }
}
```

> **设计原则**：manifest 中每条记录自描述，包含所有下游需要的信息。ID 即目录名（`php82`），`env_prefix` 由 `service_dir.to_uppercase()` 推导（`PHP82`），无需运行时格式转换。

---

## 4. 数据流概览

### 4.1 配置生成数据流

```
用户输入 (GUI)
    ↓
EnvConfig { services: [ServiceEntry { version: "php82", ... }] }
    ↓
ConfigGenerator.generate_env(config, project_root)
    ↓
VersionManifest.get_entry("php", "php82")
    → VersionEntry { image_tag: "php:8.2-fpm", service_dir: "php82", ... }
    ↓
UserOverrideManager.get_merged_entry("php", "php82")
    → 合并用户自定义 image_tag（如有）
    ↓
env_prefix = entry.service_dir.to_uppercase()  // "PHP82"
env.set("PHP82_VERSION", entry.image_tag)       // 直接使用，零转换
    ↓
写入 .env → 写入 docker-compose.yml → 创建 services/ 目录
```

### 4.2 版本映射查询数据流

```
前端 invoke('get_version_mappings')
    ↓
commands.rs → VersionManifest + UserOverrideManager
    ↓
返回 { php: [{ id: "php85", display_name: "PHP 8.5", image_tag: "php:8.5-fpm", ... }] }
    ↓
前端下拉列表：value=v.id, 显示 v.display_name → v.image_tag
```

---

## 5. 文件结构

```
php-stack/
├── src/                              # 前端代码
│   ├── App.vue                       # 主框架（日志面板、导航）
│   ├── components/
│   │   ├── EnvConfigPage.vue         # 环境配置
│   │   ├── MirrorPanel.vue           # 镜像源管理
│   │   ├── BackupPage.vue            # 环境备份
│   │   ├── RestorePage.vue           # 环境恢复
│   │   ├── SoftwareSettings.vue      # 软件设置/版本管理
│   │   ├── ConfirmDialog.vue         # 确认弹窗
│   │   ├── Toast.vue                 # 提示组件
│   │   └── WorkspaceInitDialog.vue   # 工作目录初始化
│   ├── composables/
│   │   ├── useConfirmDialog.ts       # 确认弹窗 composable
│   │   └── useToast.ts              # Toast + 日志状态管理
│   ├── types/
│   │   └── env-config.ts            # 统一类型定义（VersionInfo, ServiceEntry 等）
│   └── utils/
│       └── portChecker.ts           # 端口冲突检测工具
├── src-tauri/
│   ├── src/
│   │   ├── commands.rs              # Tauri 命令入口
│   │   ├── lib.rs                   # 插件注册与日志初始化
│   │   ├── logging.rs               # 日志基础设施
│   │   ├── engine/
│   │   │   ├── version_manifest.rs  # 版本清单（VersionEntry）
│   │   │   ├── user_override_manager.rs # 用户覆盖管理
│   │   │   ├── config_generator.rs  # 配置生成器
│   │   │   ├── env_parser.rs        # .env 解析器
│   │   │   ├── mirror_manager.rs    # 镜像源管理
│   │   │   ├── workspace_manager.rs # 工作目录管理
│   │   │   ├── backup_engine.rs     # 备份引擎
│   │   │   ├── restore_engine.rs    # 恢复引擎
│   │   │   └── backup_manifest.rs   # 备份清单模型
│   │   └── docker/
│   │       ├── manager.rs           # 容器管理（bollard）
│   │       └── mirror.rs            # 镜像源切换
│   └── services/
│       ├── version_manifest.json    # 版本清单数据
│       ├── php56/ ~ php85/          # PHP 服务模板（8 个版本）
│       ├── mysql57/ ~ mysql84/      # MySQL 服务模板（3 个版本）
│       ├── redis62/ ~ redis82/      # Redis 服务模板（4 个版本）
│       └── nginx124/ ~ nginx128/    # Nginx 服务模板（5 个版本）
├── .env                             # 生成的环境变量
├── docker-compose.yml               # 生成的 Compose 文件
├── .user_version_overrides.json     # 用户版本覆盖配置
└── workspace.json                   # 工作目录配置
```

---

## 6. 子文档索引

| 文档 | 内容 | 说明 |
|------|------|------|
| [WORKFLOWS.md](./WORKFLOWS.md) | 核心工作流程 | 工作目录初始化、环境配置与启动、版本映射查询、备份、恢复 |
| [LOGGING.md](./LOGGING.md) | 日志与启动系统 | 三层日志架构、实时日志推送、容器启动智能等待机制 |
| [DECISIONS.md](./DECISIONS.md) | 关键技术决策 | 版本清单设计、用户覆盖机制、ZIP 备份、动态镜像切换等 ADR |
| [EXTENSION_GUIDE.md](./EXTENSION_GUIDE.md) | 扩展指南 | 添加新版本、用户自定义标签、添加新 Command |

### 相关文档

- [VERSION_MANIFEST.md](../history/2026-04-20_VERSION_MANIFEST.md) — 版本清单系统历史说明
- [IMPLEMENTATION_SUMMARY.md](../implementation/IMPLEMENTATION_SUMMARY.md) — 实现总结报告
- [DYNAMIC_BASE_IMAGE.md](../guides/DYNAMIC_BASE_IMAGE.md) — 动态基础镜像详细文档
- [TESTING_GUIDE.md](../guides/TESTING_GUIDE.md) — 测试指南

---

**文档维护**: 每次重大架构变更时，请同步更新此文档及相关子文档。
