# PHP-Stack 开发者指南

> **面向对象**: 参与 PHP-Stack 开发的开发者 / AI Agent
> **配套文档**: [README.md](README.md)（用户视角）、[AGENTS.md](AGENTS.md)（AI Agent 协作规范）、[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)（系统架构）

本文档说明如何为 PHP-Stack 贡献代码：环境准备、常用命令、开发与测试规范、扩展指南。

---

## 1. 项目概览

PHP-Stack 是一个基于 **Tauri v2 + Docker** 的跨平台 PHP 开发环境可视化管理工具（当前 v0.4.0）。

| 层级 | 技术 | 说明 |
|------|------|------|
| 前端 | Vue 3 + TypeScript + Tailwind CSS v4 | UI 界面 |
| 后端 | Rust (Tauri v2) + bollard | 系统级操作与 Docker API |
| 容器 | Docker + Docker Compose | 环境隔离 |
| 测试 | cargo test（Rust）+ Vitest（前端） | 单元/集成/组件测试 |

## 2. 环境准备

1. **Node.js**（≥ 20，需含 npm）
2. **Rust 工具链**（rustup，含 cargo）
3. **Docker Desktop**（运行中，`ps-` 前缀容器由本项目管理）
4. **Windows** 需 [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)（含 C++ 构建工具）；macOS/Linux 按 Tauri v2 前置要求安装

安装依赖：

```bash
npm install
```

## 3. 常用命令

### 3.1 开发与构建

```bash
npm run tauri dev          # 启动开发模式（前后端热更新）
npm run build              # 前端构建（vue-tsc 类型检查 + vite build）
npm run tauri build        # 打包应用（Windows: MSI/NSIS）
scripts/build.ps1          # Windows 打包辅助脚本（产物路径见脚本输出）
```

### 3.2 Rust 后端

```bash
cd src-tauri
cargo check                # 快速编译检查
cargo test                 # 全部测试（单元 + 集成）
cargo test --lib           # 仅单元测试
cargo test --test '*'      # 仅集成测试
cargo test <test_name>     # 运行特定测试
cargo clippy --all-targets -- -D warnings   # lint（CI 强制执行）
cargo fmt --check          # 格式检查（CI 强制执行）
```

### 3.3 Vue 前端

```bash
npm run test               # 测试（监视模式）
npm run test:run           # 测试（运行一次）
npm run test:ui            # 测试 UI 界面
npm run test:coverage      # 覆盖率报告
```

### 3.4 检查脚本

```bash
npm run check:i18n         # i18n key 检查（重复 key、中英一致性、源码引用）
npm run check:manifest     # 版本清单同步检查（有差异时 exit 1，供 CI）
npm run check:dockerfile   # PHP Dockerfile 模板一致性检查
```

### 3.5 版本清单 / 模板同步（仓库侧，不进入 App 运行时）

```bash
npm run sync:manifest      # 从上游同步版本元数据并写回 version_manifest.json（仅新增）
node scripts/sync-version-manifest.mjs --offline --apply   # 离线模式（复用缓存）
npm run sync:dockerfile    # 同步 PHP Dockerfile 参数化模板
```

> 日常可用「软件设置 → 新增版本」写入工作区自定义条目（无需重编译）。开发者改仓库 `version_manifest.json` / 模板后须重新构建（`include_str!`）。同步工作流详见第 7 节。设计原则：**App 保持离线可用**，同步只发生在仓库侧（开发者本地或 CI），产物随版本发布。

## 4. 目录结构

```
php-stack/
├── src/                        # 前端
│   ├── App.vue                 # 主框架（侧边栏、日志面板、容器状态）
│   ├── components/             # 页面与通用组件（EnvConfigPage / MirrorPanel / BackupPage / RestorePage / SoftwareSettings / AboutPage / SettingsPage / CustomSelect / UiTabs / ...）
│   ├── composables/            # 状态与逻辑（useToast / useConfirmDialog / useDocker）
│   ├── api/                    # 后端命令封装（client / envConfig / mirror / backup / workspace / docker）
│   ├── types/                  # 与 Rust 结构体对应的 TypeScript 类型
│   └── i18n/locales/           # zh-CN.json / en.json 语言包
├── src-tauri/
│   ├── src/
│   │   ├── commands/           # #[tauri::command] 入口，按业务域拆分（docker/env_config/mirror/backup/workspace/app + paths/mod）
│   │   ├── engine/             # 核心业务引擎（config_generator / version_manifest / user_override_manager / site_manager / user_config / backup_* / restore_engine / config_extractor / ...）
│   │   ├── docker/             # Docker 交互层（manager / mirror）
│   │   ├── logging.rs          # 日志系统（文件轮转 + tracing）
│   │   └── macros.rs           # app_log! / ui_log! 宏
│   └── services/               # 服务模板与版本清单
│       ├── version_manifest.json   # 版本清单（include_str! 编译进二进制）
│       └── php*/ mysql*/ redis*/ nginx*/   # 各服务模板目录
├── scripts/                    # 同步与检查脚本（sync-version-manifest / sync-php-dockerfile / check-i18n-keys / build）
├── .github/workflows/          # CI（fmt/clippy/test/build）与 release
├── docs/ARCHITECTURE.md        # 系统架构文档（唯一常驻 docs 文档）
├── AGENTS.md                   # AI Agent 协作指南
└── DEV.md                      # 本文档
```

## 5. 开发规范

### Rust 后端

1. **错误处理**: 统一 `Result<T, String>` 或 `Result<T, Box<dyn Error>>`；暴露给前端的 Command 必须将错误转为 `String`。
2. **异步**: 涉及 Docker 或文件 IO 的操作使用 `async/await`；**同步阻塞调用**（`Command::output()`、`child.wait()` 等）必须包进 `tokio::task::spawn_blocking`，避免阻塞 Tauri async 线程。
3. **模块注册**: 新引擎模块在 `engine/mod.rs` 声明；新命令子模块在 `commands/mod.rs` 声明并 re-export；新 `#[tauri::command]` 在 `lib.rs` 的 `invoke_handler` 注册。
4. **路径解析**: 统一走 `commands/paths.rs`（`project_root()` / `app_config_dir()` / `log_file()`），禁止在业务代码中自行爬目录。
5. **容器状态契约**: 使用前后端共享枚举（`ContainerState`：`running/exited/...`），禁止用 `format!("{:?}")` 字符串做契约。

### Vue 前端

1. **后端调用**: 一律经过 `src/api/` 封装（`invokeCommand` + `normalizeError`），业务组件不直接 `invoke`。
2. **类型同步**: 修改 Rust 数据结构后，同步更新 `src/types/` 对应类型（serde 字段名 ↔ TS 接口）。
3. **样式**: Tailwind CSS v4；组件内 `@apply` 需在 `<style scoped>` 声明 `@reference "tailwindcss";`。
4. **i18n**: 用户可见文案全部走 `t()` key；新增 key 必须同步 zh-CN 与 en 语言包，并跑 `npm run check:i18n` 验证。

## 6. 测试规范

### 位置与命名

| 类型 | 位置 | 命名 |
|------|------|------|
| Rust 单元测试 | 源文件内 `#[cfg(test)] mod tests` | `test_功能描述`（snake_case） |
| Rust 集成测试 | `src-tauri/tests/integration/` | `{模块}_integration.rs` |
| Vue 组件/Composable/工具测试 | 与被测代码同级 `__tests__/` | `{模块名}.spec.ts` |

### 编写要求

1. **真断言**: 禁止 `assert!(true)` 占位；解析/生成类纯函数应抽为可直接测试的函数。
2. **AAA 模式**: Arrange → Act → Assert；测试独立、可重复。
3. **临时目录**: 涉及文件操作的测试使用 `tempfile::TempDir`。
4. **异步测试**: 使用 `#[tokio::test]`。
5. **Mock 外部依赖**: 前端 Mock Tauri API；不依赖真实 Docker 环境。

### 覆盖目标

- Rust 核心逻辑 ≥ 80% 行覆盖率
- Vue 关键组件 ≥ 70%、工具函数 100%、Composables ≥ 80%

### 交付前必须全绿

```bash
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check
npm run test:run
npm run build
```

## 7. 版本清单与模板体系（扩展指南）

### 7.1 如何添加新的服务版本

三步流程（详见架构文档 §模板体系）：

1. **清单加条目**: 编辑 `src-tauri/services/version_manifest.json`，键名即 ID（如 `nginx129`），字段 `display_name / image_tag / service_dir / default_port / show_port / eol / description`。`env_prefix` 由 `service_dir.to_uppercase()` 自动推导，无需配置。
2. **准备模板目录**: 三选一——复用已有目录（`service_dir` 指向旧目录，推荐）、依赖自动回退（PHP→`php85`、MySQL→`mysql80`、Redis→`redis72`、Nginx→`nginx127`）、或复制同类型目录新建。
3. **验证**: 重新构建（清单 `include_str!` 编译进二进制，仅重启不生效）→ 环境配置页下拉出现新版本 → 应用配置检查 `services/<dir>/` 释放 → 启动容器 `ps-<dir>` 正常。

> **注意**: 清单改动必须重新构建才生效；模板文件在「应用配置」时首次释放到工作区，**目标文件已存在则跳过**（保留用户修改）。

### 7.2 用户覆盖与自定义版本（L3）

用户数据只写在工作区 `.user-config/version_overrides.json`，不改内置清单。每条记录 `entry_kind` 必填：

- `override`：映射表「编辑」已有 ID，`image_tag` + 可选 `description`，叠在清单基线上
- `custom`：映射表「新增」，完整 VersionEntry 字段，可出现在环境配置下拉

### 7.3 新增服务类型（如 PostgreSQL）Checklist

当前服务类型写死为 PHP / MySQL / Redis / Nginx。新增一种需同步：

- **后端**: `engine/config_generator.rs` 的 `ServiceType` 枚举与生成分支；`engine/version_manifest.rs` 的 `ServiceType` + `ManifestFile`；`services/version_manifest.json` 新块；`services/<newtype>/` 模板（如有）；集成测试
- **前端**: `src/types/env-config.ts` 的 `ServiceType` / `ServiceTypeLower` / `VersionMappings`；`EnvConfigPage.vue` 服务面板（长期目标是配置驱动收敛）；`SoftwareSettings.vue` 的 `serviceLabels` 与 tab；i18n `envConfig.<service>.*` / `software.*` 中英 key

> **不做**: 远程自动拉取服务定义、通用插件系统（违背「简单」原则）。

### 7.4 版本清单同步（sync-version-manifest）

- `npm run check:manifest`：CI 用，有差异 exit 1
- `npm run sync:manifest`：写回（**仅新增条目**，不动已有）

**设计红线**:
1. 永不删除 manifest 已有条目（用户手工维护的可能被覆盖，只增不改）
2. 新增条目 `service_dir` 与版本 ID 一致（避免 `.env` 反查歧义）
3. patch 升级 / EOL 标定不自动，需人工 review
4. `--apply` 模式下 fetch 失败必须报错退出，禁止写半截不一致的 manifest

## 8. 贡献流程

1. **理解 Scope**: 确认是前端 UI 还是后端 Rust 逻辑变更；涉及 Docker 修改先 `check_docker`。
2. **TDD**: 优先编写测试 → 实现 → 跑全量测试。
3. **权限**: 新增 Tauri 插件调用时，更新 `src-tauri/capabilities/default.json`。
4. **类型同步**: 修改 Rust 数据结构后同步 `src/types/`。
5. **CHANGELOG**: 每个功能/修复提交同时更新 `CHANGELOG.md` 的 `[Unreleased]` 段（新增/修复/改进/文档分类）；发版时将内容移入对应版本小节并 bump `package.json` / `src-tauri/Cargo.toml` / `tauri.conf.json`。
6. **CI 检查**: `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`、`npm run test:run`、`npm run build` 全绿（`.github/workflows/ci.yml` 自动执行）。
7. **文档**: 架构级变更更新 `docs/ARCHITECTURE.md`；面向用户的变更更新 `README.md`；不再维护的旧文档**直接删除**（git 历史可追溯），不保留归档目录。

## 9. 文档体系

项目刻意保持精简，**不设历史归档目录**：

| 文档 | 位置 | 用途 |
|------|------|------|
| README.md | 根目录 | 项目介绍、用户快速开始 |
| DEV.md | 根目录 | 本文档，开发者贡献指南 |
| AGENTS.md | 根目录 | AI Agent 协作规范（面向自动化开发） |
| CHANGELOG.md | 根目录 | 变更记录（Keep a Changelog 格式） |
| ARCHITECTURE.md | docs/ | 系统架构与技术设计（唯一常驻 docs 文档） |

> 原则：变更记录进 CHANGELOG，技术细节进 ARCHITECTURE，过时内容直接删除——不保留 `docs/history/` 之类的归档（git 本身即历史）。

---

**维护者**: PHP-Stack Team
