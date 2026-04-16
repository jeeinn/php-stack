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
    - `export.rs`: 深度导出引擎，处理 ZIP 压缩、SQL 导出及通配符匹配。
  - `commands.rs`: 定义暴露给前端的 `#[tauri::command]` 接口。
  - `lib.rs`: 插件注册与指令分发中心。

## 🛠️ 开发规范

### Rust 后端
1. **错误处理**: 统一使用 `Result<T, String>` 或 `Result<T, Box<dyn Error>>`。暴露给前端的 Command 必须将错误转换为 `String`。
2. **异步处理**: 涉及 Docker 或文件 IO 的操作必须使用 `async/await`。
3. **测试**: 核心逻辑应在 `docker/tests.rs` 或对应的测试文件中编写单元测试。

### Vue 前端
1. **状态管理**: 目前使用 `ref` 和 `reactive` 进行局部状态管理。
2. **样式**: 严格遵循 Tailwind CSS v4 规范。在组件内使用 `@apply` 时必须在 `<style scoped>` 中声明 `@reference "tailwindcss";`。
3. **交互**: 所有后端调用必须经过 `invoke` 封装，并处理 `loading` 和 `error` 状态。

## 📋 关键模块逻辑

### 1. 容器识别
系统仅识别以 `ps-` 为前缀的容器。
- 过滤逻辑位于 `src-tauri/src/docker/manager.rs`。

### 2. 导出引擎
- 导出包格式为 `.zip`。
- 包含 `manifest.json` 用于记录备份元数据。
- 路径匹配使用 `glob` 库。

## 🚀 Agent 任务接入建议

如果你被分派了新任务，请遵循以下流程：
1. **理解 Scope**: 确认是前端 UI 调整还是后端 Rust 逻辑变更。
2. **安全检查**: 涉及 Docker 修改的操作应先调用 `check_docker` 指令。
3. **权限校验**: 若新增了 Tauri 插件调用，请务必更新 `src-tauri/capabilities/default.json`。
4. **TDD 流程**: 优先在 `src-tauri/src/docker/tests.rs` 中编写测试用例。

## 🗺️ 后续开发重点 (给下个 Agent 的 Tip)

### 当前优先级任务（按顺序执行）：

#### 1. v1.1 - 软件管理中心（最高优先级）
- **目标**：实现多版本 PHP/MySQL/Redis 的一键安装与管理
- **需要新增文件**：
  - `src-tauri/src/engine/software_manager.rs` - 核心管理软件安装/卸载逻辑
  - `src/components/SoftwareCenter.vue` - 前端软件中心界面
- **关键功能**：
  - 维护可用软件版本清单（从 Docker Hub 拉取或本地缓存）
  - 根据用户选择生成并启动容器（处理端口冲突、卷挂载）
  - 支持自定义镜像标签和启动参数
  - 实时显示安装进度和日志
- **技术要点**：
  - 使用 `bollard::CreateContainerOptions` 动态创建容器
  - 端口自动分配算法（检测可用端口）
  - 数据卷持久化路径管理

#### 2. v1.2 - 虚拟主机管理
- **目标**：GUI 配置 Nginx 站点，自动处理目录挂载
- **需要新增文件**：
  - `src-tauri/src/engine/vhost_manager.rs` - 虚拟主机配置管理
  - `src/components/VhostManager.vue` - 前端虚拟主机管理界面
- **关键功能**：
  - 添加/编辑/删除 Nginx 站点配置
  - 自动生成 Handlebars 模板渲染的 `.conf` 文件
  - 自动重启 Nginx 容器使配置生效
  - 支持 SSL 证书绑定
- **技术要点**：
  - 使用 `handlebars` 库渲染 Nginx 配置模板
  - Docker 卷挂载点动态映射
  - 配置文件热重载（`nginx -s reload`）

#### 3. v1.3 - 一键导入恢复
- **目标**：实现导出包的完整还原
- **需要新增文件**：
  - `src-tauri/src/engine/import.rs` - ZIP 解压与环境还原引擎
  - `src/components/ImportWizard.vue` - 前端导入向导界面
- **关键功能**：
  - 解析 `manifest.json` 获取备份元数据
  - 自动拉取所需 Docker 镜像
  - 还原配置文件到正确位置
  - 执行 SQL 脚本恢复数据库
  - 智能处理环境差异（如端口冲突）
- **技术要点**：
  - ZIP 解压与文件权限保持
  - Docker 镜像批量拉取与错误重试
  - 事务性恢复（失败时回滚）

### 开发建议：
1. **按顺序开发**：先完成软件管理中心，再开发虚拟主机，最后实现导入恢复
2. **测试策略**：每个模块完成后进行端到端测试，确保可以一边开发一边验证
3. **依赖关系**：
   - 虚拟主机依赖已安装的 Nginx 容器（由软件管理中心提供）
   - 导入恢复需要读取软件版本信息（依赖软件管理中心的版本清单）
