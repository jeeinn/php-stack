# PHP-Stack

> 💡 **灵感来源**: 本项目设计灵感来自于 [dnmp](https://github.com/yeszao/dnmp) 项目，感谢其优秀的架构设计理念。

一个轻量级、跨平台的 PHP 开发环境可视化管理工具（v0.4.0）。

基于 **Tauri v2** + **Docker**，提供 GUI 界面用于环境配置、站点挂载、镜像源管理、备份恢复，帮助开发者快速搭建和迁移 PHP 开发环境。

## ✨ 核心功能

- **🎨 可视化环境配置** - GUI 选择服务版本、端口、扩展，自动生成 `.env` 和 `docker-compose.yml`，支持多 PHP 版本独立服务
- **🌐 Nginx 站点管理** - 多站点 `server_name`、宿主机目录挂载、`public_dir`、绑定 PHP/Nginx 服务，自动生成托管 conf
- **🌐 统一镜像源管理** - 5 个预设方案（阿里云/清华/腾讯云/中科大/官方），一键加速 Docker Registry/APT/Composer/NPM
- **💾 环境备份与恢复** - ZIP 打包 + SHA256 校验；支持站点路径跨机重映射与恢复前回滚包
- **🛠️ 工作区与版本管理** - 多工作区；清单驱动的版本映射；用户可覆盖镜像 tag 或新增自定义版本
- **🌍 国际化（i18n）** - 中/英双语支持，运行时动态切换
- **🎨 主题预设** - 自动/明亮/暗黑三种模式，全组件适配
- **🐳 跨平台权限映射** - PUID/PGID 用户映射，解决 Docker 挂载目录权限问题
- **🔄 关于页与自动更新** - 日志等级/导出、检查更新（GitHub Releases）
- **⚡ 极致轻量** - Rust 后端，安装包体积仅为 Electron 应用的 1/10

## 📚 详细文档

- **[📖 开发者指南](DEV.md)** - 环境准备、常用开发/测试命令、贡献指南
- **[🏗️ 系统架构](docs/ARCHITECTURE.md)** - 系统架构、设计决策、核心流程
- **[🤖 AI Agent 指南](AGENTS.md)** - AI 协作开发规范
- **[📋 变更记录](CHANGELOG.md)** - Keep a Changelog 格式

## 🛠️ 技术栈

- **后端**: Rust (Tauri v2), bollard (Docker SDK)
- **前端**: Vue 3, TypeScript, Tailwind CSS v4
- **测试**: 单元测试 + 集成测试 + 前端组件测试（Vitest），覆盖核心模块与关键流程

## 🚀 快速开始

### 前置要求

1. [Docker Desktop](https://www.docker.com/products/docker-desktop/) 已安装并运行
2. [Node.js](https://nodejs.org/) 环境（≥ 20）
3. (Windows) [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)

### 开发模式

```bash
# 安装依赖
npm install

# 启动开发服务器
npm run tauri dev
```

### 运行测试

```bash
# Rust 后端测试
cd src-tauri && cargo test

# 前端测试与构建
npm run test:run
npm run build
```

更多命令见 [DEV.md](DEV.md)。

## 📄 开源协议

[MIT](LICENSE)
