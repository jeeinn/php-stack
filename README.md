# PHP-Stack

> 💡 **灵感来源**: 本项目设计灵感来自于 [dnmp](https://github.com/yeszao/dnmp) 项目，感谢其优秀的架构设计理念。

一个轻量级、跨平台的 PHP 开发环境可视化管理工具（v0.1.0）。

基于 **Tauri v2** + **Docker**，提供 GUI 界面用于环境配置、镜像源管理、备份恢复，帮助开发者快速搭建和迁移 PHP 开发环境。

## ✨ 核心功能

- **🎨 可视化环境配置** - GUI 选择服务版本、端口、扩展，自动生成 `.env` 和 `docker-compose.yml`
- **🌐 统一镜像源管理** - 一键加速 APT/Composer/NPM 镜像源
- **💾 环境备份与恢复** - ZIP 格式打包，SHA256 完整性校验，支持配置文件和数据还原
- **⚡ 极致轻量** - Rust 后端，安装包体积仅为 Electron 应用的 1/10

## 📚 详细文档

完整的技术文档已整理至 `doc/` 目录：

- **[📖 文档中心](doc/README.md)** - 完整的文档索引和导航
- **[🏗️ 系统架构](doc/architecture/ARCHITECTURE.md)** - 系统架构、工作流程、模块说明
- **[💻 实现总结](doc/history/2026-04-17_IMPLEMENTATION_SUMMARY.md)** - v0.1.0 功能实现详情
- **[📖 使用指南](doc/guides/MIRROR_GUIDE.md)** - 镜像源配置、快速参考
- **[📜 历史记录](doc/history/)** - 问题修复、重构记录、开发日志

## 🛠️ 技术栈

- **后端**: Rust (Tauri v2), bollard (Docker SDK)
- **前端**: Vue 3, TypeScript, Tailwind CSS v4
- **测试**: 72 个单元测试全部通过（含属性测试）

## 🚀 快速开始

### 前置要求

1. [Docker Desktop](https://www.docker.com/products/docker-desktop/) 已安装并运行
2. [Node.js](https://nodejs.org/) 环境
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

# 前端构建
npm run build
```

## 📄 开源协议

[MIT](LICENSE)
