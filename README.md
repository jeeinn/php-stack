# PHP-Stack

一个轻量级、高性能的跨平台 PHP 开发环境可视化管理工具。

## 🚀 项目简介

`php-stack` 旨在解决 PHP 开发者在不同机器间迁移环境的痛苦。它基于 **Tauri v2** 和 **Docker**，提供了一个极致轻量的 GUI 界面，让你可以可视化地管理容器状态、一键加速国内镜像源，并支持深度打包导出开发环境。

## ✨ 核心特性

- **极致轻量**：基于 Rust 后端，安装包体积仅为传统 Electron 应用的 1/10。
- **可视化管理**：自动识别并管理 `ps-` 前缀的 Docker 容器，支持一键启停。
- **镜像源加速**：内置国内主流 Docker 镜像源，并支持一键切换 PHP 容器内的 `apt` 和 `composer` 源。
- **深度导出引擎**：支持选择性打包配置文件、MySQL 数据库（结构/数据）以及通过通配符指定的项目敏感文件（如 `.env`）。
- **环境自检**：启动时自动检测 Docker 运行状态，确保环境可用。

## 🛠️ 技术栈

- **后端**: Rust (Tauri v2), bollard (Docker SDK), zip, chrono, glob
- **前端**: Vue 3, TypeScript, Tailwind CSS v4, Vite

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

## 🗺️ 后续版本规划 (Roadmap)

### v1.1 - 软件管理中心
- 提供 PHP (5.6 - 8.4)、MySQL (5.7 - 8.0)、Redis、MongoDB 等多版本的一键安装与卸载。
- 容器化参数（端口、数据卷）的图形化配置。
- 支持自定义 Docker 镜像标签和启动参数。

### v1.2 - 虚拟主机 (Vhosts)
- 支持 GUI 添加 Nginx 站点配置。
- 自动处理宿主机与容器间的目录挂载映射。
- 支持 SSL 证书绑定与自动续期（Let's Encrypt）。

### v1.3 - 快速恢复 (Import)
- 实现备份包 (.zip) 的一键导入。
- 自动还原配置文件、项目敏感文件及 MySQL 数据库。
- 智能检测环境差异并自动调整（如端口冲突处理）。

## 📄 开源协议
[MIT](LICENSE)
