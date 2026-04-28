# Changelog

所有重要的项目变更都将记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
并遵循 [语义化版本](https://semver.org/lang/zh-CN/) 规范。

## [Unreleased]

### 待发布功能
- 虚拟主机管理（Nginx 站点配置）

---

## [0.3.0] - 2026-04-28

PHP-Stack v0.3.0 正式发布！此版本重点增强了用户体验和国际化支持，引入了完整的 i18n 多语言系统和主题预设功能。

### ✨ 新增功能

#### 前端国际化 (i18n) 支持
- ✅ 添加 vue-i18n 依赖并配置中/英双语支持
- ✅ 创建 i18n 目录结构及翻译文件 (zh-CN.json, en.json)
- ✅ 改造所有 Vue 组件使用 $t() 函数替换硬编码文本
- ✅ 后端所有日志消息统一改为英文，便于国际化部署
- ✅ 支持运行时动态切换语言

#### 主题预设系统
- ✅ 新增 useTheme composable，支持自动/明亮/暗黑三种主题模式
- ✅ 完善 Tailwind darkMode 配置和全局样式过渡动画
- ✅ 适配 11 个主要组件的明亮/暗黑主题样式：
  - App.vue - 主应用布局
  - SettingsPage.vue - 设置页面（含主题切换器 UI）
  - MirrorPanel.vue - 镜像源设置
  - SoftwareSettings.vue - 软件版本映射
  - EnvConfigPage.vue - 环境配置页面
  - MigrationPage.vue - 迁移页面
  - BackupPage.vue - 备份页面
  - RestorePage.vue - 恢复页面
  - CustomSelect.vue - 自定义下拉选择器
  - ConfirmDialog.vue - 确认对话框
- ✅ 添加主题相关的国际化支持（zh-CN/en）

### 🐛 Bug 修复

#### 主题适配修复
- ✅ 修复环境配置页面 Nginx 配置提醒的深色模式主题适配
  - 外层提示框、代码块背景、多 Nginx 警告框的颜色适配
  - 确保浅色和深色模式下都有足够的对比度
- ✅ 修复时区提示文本的国际化遗漏

#### 国际化修复
- ✅ 修复 Docker Registry 配置文档数组翻译显示问题
  - 使用 tm() 替代 t() 获取 vue-i18n v10 数组类型翻译
  - 统一 Windows/macOS/Linux 三个平台的 JSON 配置描述格式

### 🔧 技术实现

#### 前端组件（Vue 3 + TypeScript）
- **i18n/index.ts** - 国际化配置和初始化
- **i18n/locales/zh-CN.json** - 中文翻译文件（490+ 词条）
- **i18n/locales/en.json** - 英文翻译文件（490+ 词条）
- **composables/useTheme.ts** - 主题管理 composable
- **style.css** - 添加暗色模式支持和过渡动画

#### 后端（Rust）
- **lib.rs** - 日志消息英文化
- **所有引擎模块** - app_log/ui_log 消息统一改为英文

### 📊 代码统计

| 模块 | 行数变化 | 主要变更 |
|------|----------|----------|
| Vue 前端组件 | ~1500行新增 | i18n 集成、主题系统、样式适配 |
| Rust 后端 | ~50行调整 | 日志英文化 |
| 翻译文件 | ~980行新增 | 中英文翻译词条 |
| **总计** | **~2780行新增** | **国际化与主题系统全面升级** |

---

## [0.2.0] - 2026-04-27

PHP-Stack v0.2.0 正式发布！此版本重点优化了代码结构、用户体验和 CI/CD 流程，引入了自定义选择组件并重构了后端命令模块。

### ✨ 新增功能

#### 自定义选择组件 (CustomSelect)
- ✅ 新增 `CustomSelect.vue` 组件，提供更美观的下拉选择体验
- ✅ 支持搜索过滤、键盘导航和无障碍访问
- ✅ 修复关键问题：选项渲染、状态同步和边界情况处理
- ✅ 在环境配置页中应用新组件，提升用户交互体验

#### 后端命令模块化重构
- ✅ 将庞大的 `commands.rs` 拆分为按业务域组织的子模块：
  - `backup.rs`: 备份相关命令
  - `docker.rs`: Docker 容器管理命令
  - `env_config.rs`: 环境配置相关命令
  - `mirror.rs`: 镜像源管理命令
  - `workspace.rs`: 工作区管理命令
- ✅ 提高代码可维护性和可读性
- ✅ 每个模块职责单一，便于测试和扩展

#### GitHub Actions 多平台发布工作流
- ✅ 添加 `.github/workflows/release.yml` 自动化发布流程
- ✅ 支持 Windows、macOS、Linux 多平台构建和发布
- ✅ 自动创建 GitHub Release 并上传安装包

#### 其他功能增强
- ✅ 在 `docker-compose.yml` 顶层添加项目名称 (`name: ${PROJECT_NAME}`)
- ✅ 优化 PHP 扩展配置 UI，支持自定义扩展输入
- ✅ 优化环境配置页时区选项，提供更完整的时区列表
- ✅ 修复 Nginx 服务名称生成逻辑，确保一致性
- ✅ 首页应用配置后立即刷新检测状态，提升响应速度
- ✅ 启动日志改为流式输出，实时显示启动过程
- ✅ `apply` 操作时始终生成全新 `.env` 文件，不保留已删除服务的旧变量
- ✅ 修复 `generate_env` 使用正确的 `project_root` 加载用户覆盖配置
- ✅ 重构 `version_manifest.json` 简化程序的版本转换逻辑

---

## [0.1.1] - 2026-04-25

### 🐛 Bug 修复

#### 启动按钮状态卡死问题修复
- ✅ 修复点击“一键启动”后按钮状态一直显示“启动中...”的问题
- ✅ 将前台模式改为后台启动 + 日志流分离架构
- ✅ 实现智能等待机制，支持长时启动场景（pull/build）
- ✅ 确保函数能正常返回，触发前端 finally 块更新按钮状态

**技术细节**:
- 新增 `DockerManager::check_all_ps_containers_running()` 方法检查容器状态
- 实现双重保险的智能等待循环：
  1. 轮询检查所有 ps- 容器是否进入 running 状态（每 2 秒）
  2. 监控 logs 进程状态，捕获 Docker 错误
- 移除固定 3 秒等待，无硬超时设计
- 保留流式日志的用户体验

**解决的核心问题**:
- `docker compose up` 不带 `-d` 会导致 `child.wait()` 永久阻塞
- 前端 `await invoke` 永不完成，finally 不执行
- 按钮状态卡在“启动中...”无法更新

**测试场景**:
- ✅ 快速重启：容器已存在，按钮状态正确更新
- ✅ 首次启动：pull/build 过程持续显示日志，完成后按钮更新
- ✅ 端口冲突：错误提示正确显示，按钮状态恢复
- ✅ 长时间运行：连续操作无状态异常

---

## [0.1.0] - 2026-04-24

PHP-Stack v0.1.0 内测版正式发布！这是一个功能完整的 PHP 开发环境可视化管理工具，包含环境配置、镜像源管理、备份恢复三大核心模块。

### ✨ 新增功能

#### 全服务动态基础镜像切换
- ✅ 统一所有服务的镜像标签格式为完整镜像名称 (image:tag)
- ✅ PHP (56-85): 支持通过 ARG + FROM 变量动态切换基础镜像
- ✅ Nginx (124/125/127): 新增动态基础镜像支持，修复硬编码问题
- ✅ MySQL/Redis: 优化配置生成，使用完整镜像标签格式
- ✅ 用户可自由切换 Debian/Alpine 或自定义镜像
- ✅ 支持版本锁定、私有仓库、测试新版等场景
- ✅ 优先级机制: .env > .user_version_overrides.json > version_manifest.json

#### 统一日志系统
- ✅ 添加 tracing + tracing-subscriber 依赖，替换原有日志方案
- ✅ 创建 logging.rs 模块，支持文件日志和控制台日志
- ✅ 创建 macros.rs 统一日志宏 (app_log! / ui_log!)
- ✅ 重构所有模块日志调用，实现统一日志格式
- ✅ 添加 export_logs 命令，支持一键复制日志
- ✅ 前端智能滚动（用户手动滚动时暂停自动滚动）
- ✅ 添加'复制日志'按钮到日志面板
- ✅ 日志文件保存在应用同级目录，便于用户查找和发送
- ✅ 日志格式：[HH:MM:SS.mmm] LEVEL [module] message

#### 容器管理增强
- ✅ 拆分容器管理按钮为启动/重启/停止三个独立按钮
- ✅ 根据容器状态动态控制按钮可用性
- ✅ 一键启动按钮在未检测到.env文件时置灰，引导用户先应用配置

#### 配置备份优化
- ✅ 将原来的文件重命名备份改为打包成ZIP文件
- ✅ 备份文件格式：config_backup_YYYYMMDD_HHMMSS.zip
- ✅ 支持递归打包services目录及其所有子文件
- ✅ 使用Deflated压缩算法减小备份文件大小
- ✅ 部分文件失败不影响整体备份（容错处理）
- ✅ 如果没有任何文件成功备份，自动删除空ZIP文件
- ✅ 配置备份增加用户配置文件支持 (.user_mirror_config.json, .user_version_overrides.json)

#### MySQL root密码自定义配置
- ✅ 在EnvConfigPage中添加MySQL root密码输入框
- ✅ 更新EnvConfig结构体支持mysql_root_password字段
- ✅ 修改config_generator使用用户配置的密码或默认'root'
- ✅ 优化commands.rs中load_existing_config逻辑，正确解析MYSQL版本键

### 🔧 技术实现

#### 后端引擎（Rust）
- **logging.rs** - 统一日志系统，支持文件和控制台输出
- **macros.rs** - 统一日志宏定义，简化日志调用
- **config_generator.rs** - 优化镜像标签生成逻辑，支持动态基础镜像
- **backup_engine.rs** - 优化备份流程，支持ZIP打包和用户配置文件备份
- **commands.rs** - 隐藏Windows下Docker子进程的控制台窗口

#### 前端组件（Vue 3 + TypeScript）
- **App.vue** - 优化侧边栏初始状态、响应式布局、日志面板功能
- **EnvConfigPage.vue** - 移除PHP服务宿主机端口配置UI，添加MySQL密码配置
- **MirrorPanel.vue** - 优化表格布局和交互体验
- **SoftwareSettings.vue** - 优化表格布局和交互体验

### 🐛 Bug 修复

#### Windows平台相关
- 修复 Windows 正式版点击一键启动时弹出黑色控制台的问题
- 为所有 docker compose 命令添加 CREATE_NO_WINDOW 标志

#### UI/UX优化
- 修复剪贴板权限问题，添加 clipboard-manager:allow-write-text 权限
- 优化日志面板自动滚动逻辑（1秒恢复）
- 新增日志面板显示时自动滚动到底部
- 新增手动滚动到底部按钮
- 优化设置页UI布局和交互体验
- 表格布局优化：固定操作栏 + 横向滚动其他列
- 菜单名称优化：设置项 → 其他设置（更清晰）
- 设置页头部优化：删除冗余标题，保留描述文本
- 标签按钮视觉层级优化：主次分明，便于用户认知
- 表格表头垂直居中修复：pb-3 → py-3，文字完美垂直居中
- 窗口最小尺寸配置：添加 minWidth: 900, minHeight: 650

#### 代码质量优化
- 自动修复 267+ 处 uninlined_format_args 警告
- 优化路径显示，使用 .display() 替代 Debug 格式化
- 清理 config_generator.rs 中重复的 8.5 版本分支
- 优化 user_override_manager.rs 类型签名 (&PathBuf -> &Path)
- 修复测试文件中的格式化字符串和断言
- 移除未使用的导入和变量
- 修复 App.vue 中 Tailwind CSS @reference 指令（恢复运行时支持）
- 警告从 303 个减少到 6 个非关键性警告

### 📝 文档改进

- 创建 DYNAMIC_BASE_IMAGE.md 详细使用文档
- 更新 ARCHITECTURE.md 架构文档，添加实时日志工作流程说明
- 更新架构文档目录结构，添加第3章所有子章节的目录链接
- 更新架构文档，说明配置备份包含用户配置文件
- 更新架构文档，添加ZIP备份机制详细说明

### 🧪 测试覆盖

- ✅ 所有 71 个单元测试通过（100% 通过率）
- ✅ 属性测试验证设计属性保持不变

### 📊 代码统计

| 模块 | 行数变化 | 主要变更 |
|------|----------|----------|
| Rust 后端引擎 | ~1500行新增 | 日志系统、镜像切换、备份优化 |
| Vue 前端组件 | ~500行调整 | UI优化、交互改进、响应式设计 |
| 文档 | ~1000行新增 | 新特性文档、架构更新 |
| **总计** | **~3000行** | **全面优化升级** |

### 🔗 相关链接

- [DYNAMIC_BASE_IMAGE.md](./doc/guides/DYNAMIC_BASE_IMAGE.md) - 动态基础镜像使用指南
- [ARCHITECTURE.md](./doc/architecture/ARCHITECTURE.md) - 系统架构文档（已更新）
- [AGENTS.md](./AGENTS.md) - AI Agent 指南
- [文档中心](./doc/README.md) - 完整文档索引

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
**最后更新**: 2026-04-27
