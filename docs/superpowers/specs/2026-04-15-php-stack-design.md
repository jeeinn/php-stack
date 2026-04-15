# PHP-Stack 设计文档 (Tauri 版)

## 1. 项目概述

`php-stack` 是一个轻量级、跨平台的 PHP 可视化开发环境管理工具。它通过 Docker 容器化技术，提供 nginx、MySQL（多版本）、PHP（多版本）、Redis（多版本）的一键安装与配置。核心目标是简化开发环境的搭建、切换以及迁移过程。

## 2. 核心特性

- **可视化管理**: 采用 Tauri + Vue 3 构建轻量级 GUI，包体积控制在 10MB 左右。
- **环境隔离**: 所有容器名称以 `ps-` 开头，避免与宿主机其他容器冲突。
- **镜像源优化**: 内置 Docker 及 PHP 扩展国内加速源，解决安装慢的问题。
- **虚拟主机配置**: 动态生成 nginx `vhost` 配置，支持一键重启生效。
- **一键导入导出**: 包含全部配置文件、项目核心文件（如 `.env`）及数据库 SQL 脚本。

## 3. 技术栈

- **Frontend**: Vue 3, Tailwind CSS, Vite
- **Backend**: Rust (Tauri)
- **Containerization**: Docker (Docker SDK for Rust - `bollard`)
- **Config Template**: Handlebars (用于生成 nginx/php 配置文件)

## 4. 详细设计方案

### 4.1 容器命名规范

所有服务容器均遵循以下命名：`ps-{service}-{version}`。

- 例如: `ps-nginx`, `ps-php-8.2`, `ps-mysql-5.7`, `ps-redis-6.2`。

### 4.2 镜像源切换逻辑

- **Docker 层**: 修改宿主机 `/etc/docker/daemon.json` (Linux/macOS) 或 Docker Desktop 配置 (Windows)。
- **PHP 扩展层**:
  - 构建阶段: 通过 Rust 后端修改 `Dockerfile`，替换 `apt/apk` 源为阿里云/中科大源。
  - Composer: 一键执行 `composer config -g repo.packagist php https://mirrors.aliyun.com/composer/`。

### 4.3 虚拟主机 (Vhost) 管理

- **存储路径**: 宿主机 `config/nginx/vhosts/*.conf`。
- **流程**: 用户在 UI 输入域名和目录 -> Rust 后端生成配置文件 -> 执行 `docker exec ps-nginx nginx -s reload`。

### 4.4 导入导出引擎

- **导出包格式**: `.zip` (压缩的 ZIP)。
- **导出可选项 (Selective Export)**: 为了控制包体积，用户可以自由勾选以下内容：
  - **php-stack 配置**: 包含全局设置、端口映射及容器版本信息。
  - **项目核心文件**: 支持通过 UI 输入文件列表，每行一个路径，支持 `*` 通配符表达式（例如：`src/config/*.env`, `docker/certs/*`）。
  - **MySQL 数据库**: 支持按数据库勾选导出，可选仅导出结构 (DDL) 或 结构+数据。
- **导出内容详情**:
  - `manifest.json`: 记录各软件版本、端口映射、容器卷挂载及导出项清单。
  - `config/`: 所有的 nginx, php, mysql 配置文件。
  - `project_meta/`: 根据通配符匹配并提取的项目核心文件。
  - `database/`: 执行 `mysqldump` 生成的 SQL 脚本。
- **导入逻辑**: 自动拉取镜像 -> 还原配置目录 -> 创建 Docker Network -> 启动容器 -> 根据清单恢复项目文件与数据库。

## 5. 成功标准

1. 可以在 Windows/macOS/Linux 上顺利安装运行。
2. 创建一个 PHP 8.2 + MySQL 5.7 环境的时间不超过 2 分钟（国内网络）。
3. 导出环境并在另一台机器导入后，项目可立即访问且数据库数据完整。

## 6. 后续扩展方向

- 集成常用开发工具（如 phpMyAdmin, RedisInsight）。
- 支持一键生成 SSL 证书 (Let's Encrypt)。

