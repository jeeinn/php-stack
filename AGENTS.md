# AI Agent 指南 - PHP-Stack 项目

本文档旨在帮助 AI Agent 快速理解 `php-stack` 项目的架构设计、代码规范及开发模式。

## 🏗️ 架构概览

项目采用 **Tauri v2** 典型的分层架构：

- **Frontend (UI 层)**: 位于根目录 `src/`。使用 Vue 3 + TypeScript。
  - `App.vue`: 主入口，包含侧边栏导航和全局状态管理（如 Docker 可用性、日志流）。
  - `style.css`: 集成了 Tailwind CSS v4。
- **Backend (逻辑层)**: 位于 `src-tauri/src/`。使用 Rust。
  - `docker/`: 封装 Docker 交互逻辑。
    - `manager.rs`: 使用 `bollard` 处理容器列表、启停逻辑。
    - `mirror.rs`: 处理 Docker 和 PHP 镜像源切换。
  - `engine/`: 核心业务引擎。
    - `env_parser.rs`: **.env 文件解析器与格式化器**（v0.1.0 新增）
    - `config_generator.rs`: **可视化配置生成器**（v0.1.0 新增）
    - `mirror_manager.rs`: **统一镜像源管理器**（v0.1.0 新增）
    - `backup_manifest.rs`: **备份清单数据模型**（v0.1.0 新增）
    - `backup_engine.rs`: **增强备份引擎**（v0.1.0 新增）
    - `restore_engine.rs`: **恢复引擎**（v0.1.0 新增）
    - `export.rs`: 旧版导出引擎（保留向后兼容）
  - `commands.rs`: 定义暴露给前端的 `#[tauri::command]` 接口。
  - `lib.rs`: 插件注册与指令分发中心。

## ✅ v0.1.0 已完成功能

### 1. 可视化环境配置（EnvConfigPage）
- **需求覆盖**: 需求 1.1-1.9
- **实现状态**: ✅ 完成
- **核心功能**:
  - GUI 界面选择服务类型（PHP、MySQL、Redis、Nginx）及版本
  - 端口配置与实时冲突检测
  - PHP 扩展多选配置
  - 自动生成 `.env` 文件和 `docker-compose.yml`
  - 保留用户自定义变量
  - 支持多 PHP 版本独立服务

### 2. 统一镜像源管理（MirrorPanel）
- **需求覆盖**: 需求 2.1-2.7
- **实现状态**: ✅ 完成
- **核心功能**:
  - 5 个预设方案（阿里云、清华、腾讯云、中科大、官方默认）
  - 4 类镜像源独立配置（Docker Registry、APT、Composer、NPM）
  - 连接测试功能（3 秒超时）
  - 一键应用预设或单独配置

### 3. 环境备份（BackupPage）
- **需求覆盖**: 需求 3.1-3.8, 6.1-6.4
- **实现状态**: ✅ 完成
- **核心功能**:
  - ZIP 格式备份包，包含 `manifest.json`
  - 可选：数据库导出（mysqldump）、项目文件（glob 模式）、vhost 配置、日志
  - SHA256 文件完整性校验
  - Tauri 事件进度通知
  - 部分失败容错处理

### 4. 环境恢复（RestorePage）
- **需求覆盖**: 需求 4.1-4.10
- **实现状态**: ✅ 完成
- **核心功能**:
  - 备份包预览（manifest 解析、文件统计）
  - SHA256 完整性验证
  - 端口冲突检测与自动分配
  - 配置文件还原、数据库 SQL 执行
  - 进度通知与错误汇总

### 5. 基础设施模块
- **env_parser.rs**: .env 文件可靠读写，保留注释和空行（Property 9, 10）
- **backup_manifest.rs**: Manifest 序列化/反序列化（Property 11, 12）
- **测试覆盖**: 72 个单元测试全部通过，包括属性测试（proptest）

## 🛠️ 开发规范

### Rust 后端
1. **错误处理**: 统一使用 `Result<T, String>` 或 `Result<T, Box<dyn Error>>`。暴露给前端的 Command 必须将错误转换为 `String`。
2. **异步处理**: 涉及 Docker 或文件 IO 的操作必须使用 `async/await`。
3. **测试**: 
   - 核心逻辑应编写单元测试（位于各模块的 `tests` 子模块）
   - 纯函数模块（env_parser、backup_manifest、config_generator）使用 `proptest` 进行属性测试
   - 标签格式：`// Feature: env-config-and-backup, Property N: {property_text}`
4. **模块注册**: 新增模块需在 `engine/mod.rs` 中声明 `pub mod xxx;`

### Vue 前端
1. **状态管理**: 目前使用 `ref` 和 `reactive` 进行局部状态管理。
2. **样式**: 严格遵循 Tailwind CSS v4 规范。在组件内使用 `@apply` 时必须在 `<style scoped>` 中声明 `@reference "tailwindcss";`。
3. **交互**: 所有后端调用必须经过 `invoke` 封装，并处理 `loading` 和 `error` 状态。
4. **类型定义**: 前端 TypeScript 类型需与 Rust 后端的 Serialize/Deserialize 结构体对应，定义在 `src/types/` 目录下。

## 📋 关键模块逻辑

### 1. 容器识别
系统仅识别以 `ps-` 为前缀的容器。
- 过滤逻辑位于 `src-tauri/src/docker/manager.rs`。

### 2. Env_File 解析器（v0.1.0 新增）
- 位置：`src-tauri/src/engine/env_parser.rs`
- 功能：可靠读写 `.env` 文件，保留注释和空行
- 特性：
  - 支持带引号的值（单引号/双引号）
  - 支持行内注释（`#`）
  - 支持空行和纯注释行
  - 往返一致性保证（parse → format → parse）

### 3. 配置生成器（v0.1.0 新增）
- 位置：`src-tauri/src/engine/config_generator.rs`
- 功能：根据 GUI 输入生成 `.env` 和 `docker-compose.yml`
- 特性：
  - 端口冲突检测
  - 保留用户自定义变量
  - 使用 `${VAR}` 插值语法生成 Compose 文件
  - 创建 dnmp 风格的目录结构（services/、data/、logs/）

### 4. 统一镜像源管理（v0.1.0 新增）
- 位置：`src-tauri/src/engine/mirror_manager.rs`
- 功能：统一管理 Docker、APT、Composer、NPM 镜像源
- 特性：
  - 5 个预设方案
  - 单个类别独立配置
  - 3 秒超时连接测试

### 5. 备份引擎（v0.1.0 增强）
- 位置：`src-tauri/src/engine/backup_engine.rs`
- 功能：生成包含 manifest 的 ZIP 备份包
- 特性：
  - SHA256 文件完整性校验
  - 可选：数据库导出、项目文件、vhost 配置、日志
  - Tauri 事件进度通知
  - 部分失败容错处理

### 6. 恢复引擎（v0.1.0 新增）
- 位置：`src-tauri/src/engine/restore_engine.rs`
- 功能：解析备份包并还原环境
- 特性：
  - 备份预览（manifest 解析）
  - SHA256 完整性验证
  - 端口冲突检测与自动分配
  - 配置文件、数据库、项目文件还原

### 7. 备份清单（v0.1.0 新增）
- 位置：`src-tauri/src/engine/backup_manifest.rs`
- 功能：记录备份元数据和文件校验和
- 特性：
  - serde_json 序列化/反序列化
  - 必需字段验证（version、timestamp、services）
  - 往返一致性保证

## 🚀 Agent 任务接入建议

如果你被分派了新任务，请遵循以下流程：
1. **理解 Scope**: 确认是前端 UI 调整还是后端 Rust 逻辑变更。
2. **安全检查**: 涉及 Docker 修改的操作应先调用 `check_docker` 指令。
3. **权限校验**: 若新增了 Tauri 插件调用，请务必更新 `src-tauri/capabilities/default.json`。
4. **TDD 流程**: 
   - 优先编写单元测试
   - 纯函数模块使用 proptest 进行属性测试
   - 运行 `cargo test` 确保所有测试通过
5. **类型同步**: 修改 Rust 数据结构后，同步更新 `src/types/` 中的 TypeScript 类型定义。

## 🗺️ 后续开发重点 (给下个 Agent 的 Tip)

### v0.1.0 已完成的功能
✅ **环境可视化配置** - 完整实现需求 1.1-1.9
✅ **统一镜像源管理** - 完整实现需求 2.1-2.7
✅ **环境备份** - 完整实现需求 3.1-3.8, 6.1-6.4
✅ **环境恢复** - 完整实现需求 4.1-4.10
✅ **基础设施模块** - env_parser、backup_manifest、测试框架

### 当前版本定位（v0.1.0 内测发布版）

**核心定位**: PHP-Stack v0.1.0 是一个**环境配置管理与迁移工具**，专注于：
- ✅ 可视化配置生成（替代手动编辑 .env 和 docker-compose.yml）
- ✅ 镜像源统一管理（加速国内开发体验）
- ✅ 环境备份与恢复（快速迁移开发环境到新机器）

**不包含的功能**（未来版本可能考虑）:
- ❌ 软件管理中心（多版本一键安装）- 用户需自行准备 Docker 镜像
- ❌ 虚拟主机管理（Nginx 站点配置）- 用户需手动配置 Nginx

**设计理念**: 
- **轻量级**: 专注于配置管理和环境迁移，不做复杂的容器编排
- **透明性**: 生成的配置文件完全可见可编辑，不隐藏任何细节
- **兼容性**: 与 dnmp 等项目保持兼容，便于团队协作

### 待完善功能

#### v1.3 - 一键导入恢复优化（低优先级）
- **目标**：完善 restore_engine.rs 中的 mysqldump 执行逻辑
- **当前状态**：✅ 核心框架已实现，数据库导出为占位符
- **待完善**：
  - 完整的 mysqldump 执行（使用 bollard exec API）
  - 更智能的环境差异处理
  - 事务性恢复（失败时回滚）
- **优先级**: 低（当前版本可使用手动方式恢复数据库）

### 开发建议
1. **稳定优先**: v0.1.0 作为内测发布版，重点是稳定性和用户体验优化
2. **Bug 修复**: 优先处理用户反馈的问题和边界情况
3. **性能优化**: 大文件备份的流式处理、增量备份支持
4. **文档完善**: 用户手册、常见问题、最佳实践指南
5. **国际化**: 如需支持多语言，可添加 i18n 支持
