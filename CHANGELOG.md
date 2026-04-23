# Changelog

所有重要的项目变更都将记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
并遵循 [语义化版本](https://semver.org/lang/zh-CN/) 规范。

## [Unreleased]

### 待发布功能
- 软件管理中心（多版本一键安装）
- 虚拟主机管理（Nginx 站点配置）

---

## [0.1.0] - 2026-04-23

PHP-Stack v0.1.0 内测版正式发布！这是一个功能完整的 PHP 开发环境可视化管理工具，包含环境配置、镜像源管理、备份恢复三大核心模块。

### ✨ 新增功能

#### 环境可视化配置
- ✅ GUI 界面选择服务类型（PHP、MySQL、Redis、Nginx）及版本
- ✅ 端口配置与实时冲突检测
- ✅ PHP 扩展多选配置
- ✅ 自动生成 `.env` 文件和 `docker-compose.yml`
- ✅ 保留用户自定义变量
- ✅ 支持多 PHP 版本独立服务
- ✅ 创建 dnmp 风格目录结构（services/、data/、logs/）
- ✅ 使用 `${VAR}` 插值语法生成 Compose 文件

#### 统一镜像源管理
- ✅ 5 个预设方案（阿里云、清华、腾讯云、中科大、官方默认）
- ✅ 4 类镜像源独立配置（Docker Registry、APT、Composer、NPM）
- ✅ 连接测试功能（3 秒超时）
- ✅ 一键应用预设或单独配置

#### 环境备份与恢复
- ✅ ZIP 格式备份包，包含 `manifest.json`
- ✅ SHA256 文件完整性校验
- ✅ 可选：数据库导出、项目文件、vhost 配置、日志
- ✅ Tauri 事件进度通知
- ✅ 部分失败容错处理
- ✅ 备份预览、端口冲突检测与自动分配
- ✅ 配置文件还原、数据库 SQL 执行

#### 通用版本化目录系统
- ✅ 智能版本化目录管理（php85, mysql57, redis72, nginx127）
- ✅ 配置按钮功能完善，可直接打开服务配置目录
- ✅ 模板复用机制，相似版本共享配置模板
- ✅ 面向未来的设计，添加新版本无需修改代码逻辑

### 🔧 技术实现

#### 后端引擎（Rust）
- **env_parser.rs** - .env 文件解析器与格式化器，保留注释和空行
- **config_generator.rs** - 可视化配置生成器，支持端口冲突检测
- **mirror_manager.rs** - 统一镜像源管理器，支持 5 个预设方案
- **backup_manifest.rs** - 备份清单数据模型，serde_json 序列化
- **backup_engine.rs** - 增强备份引擎，SHA256 完整性校验
- **restore_engine.rs** - 恢复引擎，支持端口冲突自动分配
- **version_manifest.rs** - 版本清单管理器，集中管理服务版本映射

#### 前端组件（Vue 3 + TypeScript）
- **EnvConfigPage.vue** - 环境配置页面
- **MirrorPanel.vue** - 镜像源管理面板
- **BackupPage.vue** - 环境备份页面
- **RestorePage.vue** - 环境恢复页面（分步卡片式交互）
- **SoftwareSettings.vue** - 软件设置/版本映射页面

### 🐛 Bug 修复

#### Docker 构建相关
- 修复 Vite 开发服务器检测到 `.env` 变化自动重启问题
- 修复配置加载时版本号解析错误（PHP56_VERSION 格式支持）
- 修复模板路径计算错误（开发模式下路径层级修正）
- 修复 Nginx Dockerfile 缺失问题
- 修复 PHP 5.6/7.4 构建失败（Debian EOL，改用 Alpine 镜像）
- 修复 PHP 扩展安装参数格式错误（逗号分隔改为空格分隔）
- 修复容器名称冲突问题（启动前自动清理旧容器）
- 修复 Tauri v2 emit 方法编译错误（导入 Emitter trait）

#### 端口冲突检测
- 实现基于 Docker API 的端口冲突检测（完全跨平台）
- 实现循环检测等待容器停止机制（最多 10 次，每次 1 秒）
- 添加 ConfirmDialog 交互规范（禁止点击外部关闭）

### 📝 文档改进

- 创建 ARCHITECTURE.md 系统架构文档
- 创建 IMPLEMENTATION_SUMMARY.md 实现总结报告
- 更新 README.md 项目说明文档
- 更新 AGENTS.md AI Agent 指南
- 创建 Dev.log.md 开发日志
- 创建多个技术文档（USER_OVERRIDE_GUIDE.md 等）

### 🧪 测试覆盖

- ✅ 72 个单元测试全部通过（100% 通过率）
- ✅ 12 个属性测试（proptest）验证设计属性
- ✅ 配置生成正确性测试（Property 1-6）
- ✅ 镜像源类别独立性测试（Property 7）
- ✅ SHA256 完整性验证测试（Property 8）
- ✅ Env_File 往返一致性测试（Property 9-10）
- ✅ Manifest 往返一致性测试（Property 11-12）

### 📊 代码统计

| 模块 | 行数 | 文件数 |
|------|------|--------|
| Rust 后端引擎 | ~150 KB | 7 个新模块 |
| Vue 前端组件 | ~40 KB | 4 个新组件 |
| TypeScript 类型 | ~2 KB | 1 个类型文件 |
| **总计** | **~192 KB** | **12 个文件** |

### 🎯 需求覆盖

- ✅ 需求 1.1-1.9: 可视化环境配置（9/9）
- ✅ 需求 2.1-2.7: 统一镜像源管理（7/7）
- ✅ 需求 3.1-3.8: 环境备份（8/8）
- ✅ 需求 4.1-4.10: 环境恢复（10/10）
- ✅ 需求 5.1-5.4: Env_File 解析器（4/4）
- ✅ 需求 6.1-6.4: Backup_Manifest（4/4）
- **总计**: 42/42 需求全部实现

### 🔗 相关链接

- [ARCHITECTURE.md](./ARCHITECTURE.md) - 系统架构文档
- [IMPLEMENTATION_SUMMARY.md](./IMPLEMENTATION_SUMMARY.md) - 实现总结报告
- [AGENTS.md](./AGENTS.md) - AI Agent 指南
- [Dev.log.md](./Dev.log.md) - 开发日志

---

## 版本说明

### 语义化版本规范

本项目遵循 [语义化版本 2.0.0](https://semver.org/lang/zh-CN/) 规范：

- **主版本号 (x)**: 不兼容的 API 修改
- **次版本号 (y)**: 向下兼容的功能性新增
- **修订号 (z)**: 向下兼容的问题修正

### 版本阶段

- **v0.x.y**: 内测阶段，API 可能不稳定
- **v1.0.0**: 正式发布，API 稳定
- **v1.x.y**: 持续迭代，保持向后兼容

### 预发布标识（未来使用）

- `-alpha`: 内部测试版，功能不完整
- `-beta`: 公开测试版，功能完整但可能有 bug
- `-rc`: 候选发布版，接近正式版

---

**维护者**: PHP-Stack Team  
**最后更新**: 2026-04-23
