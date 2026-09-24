# Changelog

所有重要的项目变更都将记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
并遵循 [语义化版本](https://semver.org/lang/zh-CN/) 规范。

## [Unreleased]

### ✨ 新增
- **容器内连接主机名提示**：环境配置页每个服务卡片展示推荐主机名（可复制）；同类多实例时警告短名不可用；应用配置后汇总连接地址
- 单实例 MySQL/Redis/Nginx 在 compose 网络上增加短主机名别名（`mysql` / `redis` / `nginx`），便于应用使用常见主机名

### 待完善（未纳入本版）
- 恢复时按建议端口自动改写 `.env` / compose（`port_overrides`）
- 数据库 mysqldump 导出与恢复时 SQL 自动导入
- 多实例时「默认实例」选型（短别名指向用户指定的默认服务）

---

## [0.4.0] - 2026-09-24

### ✨ 新增
- **Nginx 站点管理**：环境配置页可添加多站点（`server_name`、宿主机挂载、`public_dir`、绑定 PHP/Nginx 服务），生成托管 conf 与 compose 卷映射；元数据落盘 `.user-config/sites.json`
- **跨机恢复路径重映射**：恢复预览支持按站点覆写宿主机路径，适配换机后的绝对路径差异
- **自定义版本映射**：`entry_kind: override | custom`；映射表可「编辑」已有 ID 的镜像 tag，或「新增」完整版本条目并进入环境配置下拉（`.user-config/version_overrides.json`）
- **用户配置收纳**：工作区侧用户文件统一到 `.user-config/`（`mirror_config.json` / `version_overrides.json` / `sites.json` / `backup.json`）
- **备份选项持久化**：备份页选项写入 `.user-config/backup.json`；支持按站点相对路径打包本地配置文件（可选全树）
- **关于页与自动更新**：侧边栏版本入口；信息可复制、日志等级过滤、导出日志、项目地址；`tauri-plugin-updater` + GitHub Releases（检查 / 下载安装 / 重启）
- 运行时配置提取（Phase 3）：应用配置前可检测本地镜像、确认后拉取，并从官方镜像 `docker create + cp` 提取默认配置
- 用户可见日志统一为带 info/warn/error 等级的英文短句，面板可按等级过滤
- `UiTabs` 等通用 UI 组件；容器卡片启停 loading / toast；应用图标圆角矩形统一

### 🐛 修复
- 一键停止改用 `docker compose stop`，保留容器并展示已停用状态
- 统一服务模板解析路径，释放后不覆盖用户修改
- 容器状态 running 文案中文化
- 恢复引擎拒绝含路径遍历条目的备份包，堵住 zip-slip 漏洞
- 修复 Nginx 多版本配置回读时非默认端口被错误重置为 80 的问题
- 修复环境配置页多 PHP 服务时自定义扩展输入互相串扰的问题
- 修复 `scripts/` 下两个同步脚本的单元测试因 shebang 被 vitest 报 SyntaxError、整个 suite 无法加载的问题
- 修复日志文件每次启动被清空、现场日志一重启即丢失的问题（改为轮转保留最近 3 份）
- 修复日志锁被毒化后每次写日志连锁 panic 的问题
- 修复备份打包大文件时整体读入内存的问题（改为 64KB 分块流式写入）
- 修复 `path.file_name().unwrap()` 对无名路径 panic 的问题
- 修复 Docker 不可用时仍每 5 秒全量轮询、且每轮都刷一条失败日志的问题
- 修复日志面板「复制」按钮复制的是后端文件日志全文、与界面显示内容对不上的问题
- 修复配置的工作区目录不存在时数据被静默写到默认目录的问题（改为先创建目录；创建失败才回退并在界面告警）
- 修复镜像拉取命令为同步执行、下载大镜像时整个窗口「未响应」的问题（改为 async + spawn_blocking）
- 修复 `envConfig` 语言节点下存在两个同名 `toast`、JSON 解析时后者静默覆盖前者，导致应用配置后的成功提示直接显示为裸 key 的问题
- 修复 Nginx 容器启动即崩溃、无限重启的问题：Dockerfile 末尾 `USER nginx` 使 master 无权限（改为 root 运行 master，worker 仍为 `nginx`）
- 修复 PUID/PGID 用户映射在默认配置下从不生效的问题
- 修复 Nginx / PHP 镜像模板对 alpine / debian 基础镜像的用户映射命令不兼容问题
- 修复生产包 CSP 缺 `unsafe-eval` 导致白屏；补充启动期错误兜底面板
- 修复恢复 ZIP 条目名校验在 CI Linux 上的平台相关误判
- 修复 clippy `unnecessary_sort_by`；清单单测不再硬编码「最新版本 ID」（避免 sync 增版后 CI 失败）

### 🔧 改进
- 环境配置页 .env 解析主体抽为纯函数并以真断言测试覆盖
- 配置生成集成测试替换为端到端断言（validate / .env / compose / 自定义变量保留）
- 清理死代码：未注册使用的占位命令、无效占位测试、未使用的 proptest 依赖
- 新增 CI 测试流水线（fmt / clippy / cargo test / 前端测试 / 构建）；Actions checkout / setup-node 升级到 v5
- 补充 MIT LICENSE 文件，声明 Cargo `license` 字段
- 恢复前自动生成回滚包（`.restore_rollback_<时间戳>.zip`），恢复出错可据此回退
- 恢复结果由「成功/失败一句话」改为返回完整明细，前端展示已恢复文件列表、逐条错误与回滚包路径
- 恢复预览端口冲突检测（展示建议端口；不自动改写配置）
- 环境启动流程的 Docker 同步调用统一走 `spawn_blocking`，不再阻塞 async 执行线程
- **容器状态改为前后端共享的枚举契约**（`running`/`exited`/...）
- **路径解析收口为 `commands/paths.rs` 单一模块**；`workspace.json` 与日志迁至 Tauri `app_data_dir`，首次启动自动从旧位置迁移
- 日志面板新增「清空」与「导出」；轮询间隔随 Docker 连续失败次数退避
- 新增 `pnpm check:i18n` / `npm run check:i18n`（`scripts/check-i18n-keys.mjs`）
- `sync-version-manifest`：新增条目 `service_dir` 与版本 ID 对齐；清单仅增不改不删
- Nginx / Redis 注入时区环境变量；重启环境使用 `up -d --force-recreate` 以正确刷新卷

### 📁 文档
- 合并 `doc/` 与 `docs/`，精简为五份常驻文档：`README.md` / `DEV.md` / `AGENTS.md` / `CHANGELOG.md` / `docs/ARCHITECTURE.md`
- 删除 `docs/history/`、`docs/guides/`、旧 `docs/architecture/*` 索引与改进报告等过时归档（以 git 历史追溯）
- 文档定位与功能描述对齐 v0.4.0（站点管理、自定义版本、`.user-config`、关于页/更新等）

---

## [0.3.1] - 2026-04-30

### ✨ 新增
- 跨平台 PUID/PGID 用户映射，解决 Docker 挂载目录权限问题

### 🐛 修复
- 修复多 PHP 版本扩展面板同时展开/收起的问题
- 修复 Windows 下 Docker 命令执行时黑窗口闪烁及管道读取异常

### 🔧 改进
- 重构环境启动逻辑，优化流式日志输出
- 添加 `.gitattributes` 统一跨平台换行符处理

---

## [0.3.0] - 2026-04-28

### ✨ 新增
- 前端国际化（i18n）：中/英双语支持，运行时动态切换
- 主题预设系统：自动/明亮/暗黑三种模式，全组件适配

### 🐛 修复
- 修复深色模式下多处组件样式适配问题
- 修复 Docker Registry 配置文档数组翻译显示问题

---

## [0.2.0] - 2026-04-27

### ✨ 新增
- 自定义下拉选择组件（CustomSelect），支持搜索过滤和键盘导航
- 后端命令模块化重构，按业务域拆分为 5 个子模块
- GitHub Actions 多平台自动发布工作流
- PHP 扩展配置 UI 优化，支持自定义扩展输入
- 启动日志改为流式输出，实时显示启动过程

---

## [0.1.1] - 2026-04-25

### 🐛 修复
- 修复"一键启动"按钮状态卡在"启动中..."的问题
- 改为后台启动 + 日志流分离架构，实现智能等待机制

---

## [0.1.0] - 2026-04-24

PHP-Stack 首个内测版，包含三大核心模块：

### ✨ 核心功能
- **环境可视化配置**：GUI 选择服务类型/版本、端口冲突检测、自动生成 `.env` 和 `docker-compose.yml`
- **统一镜像源管理**：5 个预设方案（阿里云/清华/腾讯云/中科大/官方），4 类镜像源独立配置
- **环境备份与恢复**：ZIP 格式备份包、SHA256 完整性校验、端口冲突检测与自动分配

### 🔧 基础设施
- 统一日志系统（tracing），支持文件日志和一键导出
- 服务模板与版本清单体系
- 跨平台桌面应用（Windows / macOS / Linux）
