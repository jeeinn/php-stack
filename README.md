# PHP-Stack

一个轻量级、高性能的跨平台 PHP 开发环境可视化管理工具（v0.1.0 内测版）。

## 🚀 项目简介

`php-stack` 旨在解决 PHP 开发者在不同机器间迁移环境的痛苦。它基于 **Tauri v2** 和 **Docker**，提供了一个极致轻量的 GUI 界面，让你可以可视化地管理容器状态、一键加速国内镜像源，并支持深度打包导出开发环境。

## ✨ 核心特性

### 🆕 v0.1.0 最新改进（通用版本化目录系统）
- **🎯 智能版本化目录管理** - 所有服务采用统一的 `{name}{major}{minor}` 命名规范
  - PHP: `php56`, `php85` （已有）
  - MySQL: `mysql57`, `mysql80` （新增，支持多版本共存）
  - Redis: `redis62`, `redis72` （新增，未来可扩展）
  - Nginx: `nginx124`, `nginx127` （新增，未来可扩展）
- **🔧 配置按钮功能完善** - 环境管理页面可直接打开对应服务的配置目录
- **📦 模板复用机制** - 相似版本共享配置模板，减少冗余
- **🚀 面向未来的设计** - 添加新版本只需创建对应目录，无需修改代码逻辑

### v0.1.0 已完成功能
- **可视化环境配置** - 3 步完成开发环境搭建，GUI 选择服务版本、端口、扩展，自动生成 `.env` 和 `docker-compose.yml`
- **统一镜像源管理** - 5 个预设方案（阿里云、清华等），一键切换 Docker/APT/Composer/NPM 镜像源
- **环境备份与恢复** - ZIP 格式备份包，包含 SHA256 完整性校验、数据库导出、配置文件还原
- **向导式搭建体验** - 支持自定义 PHP 扩展安装，开箱即用！详见 [快速启动指南](./QUICKSTART-V2.md)

### 原有特性
- **极致轻量**：基于 Rust 后端，安装包体积仅为传统 Electron 应用的 1/10。
- **可视化管理**：自动识别并管理 `ps-` 前缀的 Docker 容器，支持一键启停。
- **镜像源加速**：内置国内主流 Docker 镜像源，并支持一键切换 PHP 容器内的 `apt` 和 `composer` 源。
- **深度导出引擎**：支持选择性打包配置文件、MySQL 数据库（结构/数据）以及通过通配符指定的项目敏感文件（如 `.env`）。
- **环境自检**：启动时自动检测 Docker 运行状态，确保环境可用。

## 🛠️ 技术栈

- **后端**: Rust (Tauri v2), bollard (Docker SDK), zip, chrono, glob, sha2, proptest（属性测试）
- **前端**: Vue 3, TypeScript, Tailwind CSS v4, Vite
- **测试**: 72 个单元测试全部通过，包括属性测试（proptest）

## 📦 安装与运行

### 准备工作
1. 确保已安装 [Docker Desktop](https://www.docker.com/products/docker-desktop/) 并启动。
2. 确保已安装 [Node.js](https://nodejs.org/) 环境。
3. (Windows) 确保已安装 [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)。

### 开发环境启动
```bash
# 安装依赖
npm install

# 启动开发服务器
npm run tauri dev
```

### 运行测试
```bash
# Rust 后端测试（72 个测试全部通过）
cd src-tauri && cargo test

# 前端构建
npm run build
```

## 🗺️ 后续版本规划 (Roadmap)

### ✅ v0.1.0 最新改进（通用版本化目录系统）

#### 🎯 核心改进：智能版本化目录管理

**设计理念**：参考 dnmp 项目最佳实践，实现高度统一、可扩展的版本化目录命名方案。

**命名规范**：`{service_name}{major_version}{minor_version}`
- **PHP**: `php56`, `php74`, `php80`, `php81`, `php82`, `php83`, `php84`, `php85`
- **MySQL**: `mysql57` (5.7), `mysql80` (8.0), `mysql84` (8.4) - 支持多版本共存
- **Redis**: `redis62` (6.2), `redis70` (7.0), `redis72` (7.2) - 未来可扩展
- **Nginx**: `nginx124` (1.24), `nginx125` (1.25), `nginx127` (1.27) - 未来可扩展

**亮点特性**：
1. ✨ **高度一致性** - 所有服务采用统一的命名格式，易于理解和记忆
2. 🚀 **零代码扩展** - 添加新版本只需创建对应目录，无需修改任何代码逻辑
3. 📦 **智能模板复用** - 相似版本共享配置模板（如 Redis 6.x/7.x 共用 redis72 模板）
4. 🎯 **精确版本隔离** - 每个版本的配置、数据、日志完全独立，互不干扰
5. 🔧 **配置按钮直达** - 环境管理页面点击“配置”按钮直接打开对应服务目录

**面向未来的设计**：
```bash
# 未来添加 MySQL 8.4，只需：
mkdir services/mysql84
cp services/mysql80/mysql.cnf services/mysql84/mysql.cnf
# 完成！无需修改任何代码

# 未来添加 Redis 8.0，只需：
mkdir services/redis80
cp services/redis72/redis.conf services/redis80/redis.conf
# 完成！配置生成器自动识别
```

#### 🔧 技术实现亮点

**1. 智能版本解析算法**
```rust
// MySQL: "5.7" -> "mysql57", "8.0" -> "mysql80"
let version_parts: Vec<&str> = service.version.split('.').collect();
let service_dir_name = format!("mysql{}{}", version_parts[0], version_parts[1]);

// Redis: "7.2-alpine" -> "redis72" (自动去除 -alpine 后缀)
let version_base = service.version.split('-').next().unwrap_or(&service.version);
let version_parts: Vec<&str> = version_base.split('.').collect();
let service_dir_name = format!("redis{}{}", version_parts[0], version_parts[1]);

// Nginx: "1.27-alpine" -> "nginx127"
let version_base = service.version.split('-').next().unwrap_or(&service.version);
let version_parts: Vec<&str> = version_base.split('.').collect();
let service_dir_name = format!("nginx{}{}", version_parts[0], version_parts[1]);
```

**2. 配置路径自动生成**
- `.env` 文件：`MYSQL_CONF_FILE=./services/mysql57/mysql.cnf`
- `docker-compose.yml`：`${DATA_DIR}/mysql57:/var/lib/mysql/:rw`
- 日志目录：`MYSQL_LOG_DIR=./logs/mysql57`

**3. 模板复用机制**
- MySQL 5.x → 使用 `mysql57/mysql.cnf`（`sql_mode=""`）
- MySQL 8.x → 使用 `mysql80/mysql.cnf`（严格模式）
- Redis 6.x/7.x → 共用 `redis72/redis.conf`
- Nginx 1.24-1.27 → 共用 `nginx127/` 模板

#### 💡 与 dnmp 项目的对比优势

| 特性 | dnmp | PHP-Stack v0.1.0 |
|------|------|----------------|
| 命名规范 | 部分版本化（mysql, mysql5） | 完全统一（mysql57, mysql80） |
| 扩展性 | 需手动维护映射关系 | 自动解析，零代码扩展 |
| 一致性 | PHP 使用版本号，其他不统一 | 所有服务统一格式 |
| 模板复用 | 每个版本独立模板 | 智能复用，减少冗余 |
| 配置直达 | ❌ 不支持 | ✅ 一键打开配置目录 |
| 学习成本 | 需要了解不同规则 | 单一规则，易于掌握 |

### ✅ v0.1.0 已完成（内测发布版）
- **环境可视化配置** - 完整实现需求 1.1-1.9
  - GUI 选择服务类型、版本、端口、扩展
  - 自动生成 `.env` 和 `docker-compose.yml`
  - 端口冲突检测、保留用户自定义变量
- **统一镜像源管理** - 完整实现需求 2.1-2.7
  - 5 个预设方案（阿里云、清华、腾讯云、中科大、官方默认）
  - 4 类镜像源独立配置（Docker Registry、APT、Composer、NPM）
  - 连接测试功能（3 秒超时）
- **环境备份** - 完整实现需求 3.1-3.8, 6.1-6.4
  - ZIP 格式备份包，包含 `manifest.json`
  - SHA256 文件完整性校验
  - 可选：数据库导出、项目文件、vhost 配置、日志
  - Tauri 事件进度通知
- **环境恢复** - 完整实现需求 4.1-4.10
  - 备份预览、SHA256 完整性验证
  - 端口冲突检测与自动分配
  - 配置文件、数据库、项目文件还原
- **基础设施模块**
  - env_parser.rs: .env 文件可靠读写
  - backup_manifest.rs: Manifest 序列化/反序列化
  - 72 个单元测试全部通过

### 🎯 v0.1.0 版本定位

**核心功能**: PHP-Stack v0.1.0 是一个**环境配置管理与迁移工具**，专注于：
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

### 🔧 待完善功能（低优先级）

#### v1.3 - 一键导入恢复优化
- ✅ restore_engine.rs 已实现核心逻辑
- 待完善：完整的 mysqldump 执行、更智能的环境差异处理、事务性恢复
- **优先级**: 低（当前版本可使用手动方式恢复数据库）

## 📄 开源协议
[MIT](LICENSE)
