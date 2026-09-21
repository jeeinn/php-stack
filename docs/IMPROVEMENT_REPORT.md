# PHP-Stack 改进建议报告

> **评审版本**: v0.3.1（2026-09-18）
> **评审方式**: 全量阅读前端（src/，约 5600 行）与后端（src-tauri/src/，约 6600 行）核心代码，结合 CI、文档、测试的量化检查
> **评审原则**: 一切建议服从四个指标——**简单、简洁、实用、可靠**。凡与"轻量级配置管理工具"定位冲突的"高级架构"，一律不推荐。
>
> **进度标记说明**（2026-09-21 更新）：每条建议标题后以 `[已完成]` / `[部分完成]` / `[未开始]` 标注当前状态；有实现偏差的条目在正文末尾附"实现偏差"说明。第二批 8 项已全部完成，详见第 6 节路线图。

---

## 0. 现状摘要

**做得好的（应保持）**：
- 三处版本号一致（package.json / tauri.conf.json / Cargo.toml = 0.3.1）
- i18n 双语言包 key 完全同步（zh-CN / en 各 382 个 key）
- Rust 测试 85 个 `#[test]`、前端 9 个 spec 共 60 个用例，env_parser（20 个）等纯函数模块测试扎实
- capabilities 权限最小化配置正确（`capabilities/default.json`）
- 备份/恢复有 manifest + SHA256 校验闭环，恢复前强制走 预览→校验→确认 三步
- 主题/语言初始化在 mount 前完成（`src/main.ts`），无闪烁

**量化数据**：

| 指标 | 数值 | 备注 |
|---|---|---|
| Rust `#[test]` | 85 | 其中 3 个为无效空测试（见 Q7） |
| proptest 属性测试 | **0** | Cargo.toml 声明了依赖但从未使用，AGENTS.md 描述失实 |
| 集成测试 | 4 个文件共 443 行 | config_generation_integration 仅 15 行 |
| 前端 console.log | 22 处 | 多为调试残留 |
| CI 工作流 | 仅 release.yml | **无 PR/push 测试流水线** |
| LICENSE 文件 | **不存在** | README 链接到 MIT LICENSE |
| 死命令 | 1 个 | `select_project_folder`（见 A7） |

---

## 1. 可靠性与安全（最高优先级）

> 「可靠」是四个指标中当前欠账最多的一项。以下按严重程度排序。

### R1.【P0·安全】恢复引擎存在 Zip-Slip 路径遍历风险 `[已完成]`

`src-tauri/src/engine/restore_engine.rs:268-287`：`extract_prefix` 将 ZIP 条目名直接 `target_dir.join(relative)` 写盘，**未校验路径是否逃逸目标目录**。恶意构造的备份包（条目名如 `../../../x` 或 Windows 绝对路径 `C:\...`）可把文件写到工作区之外的任意位置。`projects/` 前缀恢复（restore_engine.rs:174）直接解到 `project_root`，同样暴露。

**修复**（约 20 行）：解压前对每个条目做 canonicalize 前缀校验——拼接后确认 `target_path` 仍在 `target_dir` 之内，并拒绝含 `..`、绝对路径、盘符的条目名。同时为 `extract_file_to_path` 补同样的防护。**配套测试**：构造恶意 ZIP 的回归测试（`test_restore_rejects_path_traversal`）。

### R2.【P0·可靠】恢复会直接覆盖现有文件，且无恢复前自动备份 `[已完成]`

恢复流程（.env、docker-compose.yml、services/）逐文件 `fs::write` 覆盖（restore_engine.rs:111-189），中途失败则留下**半恢复状态**，用户当前可用环境被破坏且无法一键回退。对一个"迁移工具"而言这是最伤信任的故障模式。

**修复**（低成本方案，不需要事务框架）：`execute_restore` 开始前，把即将被覆盖的文件打包成一份自动备份 zip（复用 BackupEngine），放在工作区 `.restore_rollback_<ts>.zip`；恢复报错时在结果中提示回滚包路径。约 30-50 行。

### R3.【P1·可靠】Docker 启动流程在 async 命令中执行阻塞 IO `[已完成]`

`src-tauri/src/commands/env_config.rs` 的 `start_environment`：
- `down_cmd.output()`（:406）、`child.wait()`（:587）为同步阻塞调用；
- 日志监控循环里 `get_compose_logs`（:11-40）每 2 秒同步执行一次 `docker compose logs` 子进程（:672），最长 5 分钟（:663）。

这些调用会阻塞 Tauri 的 async 执行线程。单个用户桌面应用目前"能用"，但同线程上的其他命令（如容器列表轮询）会被拖慢，且未来任何并发操作都会受害。

**修复**：把这 3 类同步 `Command` 调用包进 `tokio::task::spawn_blocking`（项目在 `commands/backup.rs:23` 已有此模式的先例，保持一致即可）。`create_backup` 里的 `spawn_blocking` + `block_on` 双层嵌套（backup.rs:23-34）可顺手简化为单层。

### R4.【P1·可靠】容器状态用 `format!("{:?}")` 字符串作为前后端契约 `[已完成]`

`src-tauri/src/docker/manager.rs:57` 把 bollard 的枚举序列化成 `"Some(RUNNING)"` 这类 Debug 字符串，前端再用 `includes('running')` 猜测解析（`src/App.vue:79-83`），后端自己也写注释承认这个格式问题（manager.rs:122）。契约脆弱，Docker 升级或 bollard 版本变化即碎。

**修复**：`PsContainer.state` 改为 `#[serde(rename_all="lowercase")]` 的枚举（`running/exited/paused/...`），前后端类型同步更新（`src/types/` 新增对应类型）。约 40 行，一劳永逸。

### R5.【P1·可靠】备份将整个文件读入内存 `[已完成]`

`src-tauri/src/engine/backup_engine.rs:123,256`：所有文件（含可选的项目文件）先 `fs::read` 全量进内存再写 ZIP。用户勾选"包含项目文件"后，单个大文件（如数据库 dump、视频素材）即可导致内存飙升。

**修复**：`add_file_to_zip` 改为 `io::copy` 流式写入（zip crate 支持 `std::io::Read` 源），SHA256 用分块读取计算。API 签名从 `&[u8]` 改为 `&Path`，约 40 行。

### R6.【P2·可靠】日志文件每次启动清空，无轮转 `[已完成]`

`src-tauri/src/logging.rs:19-24` 使用 `truncate(true)`，每次启动覆盖 `php-stack.log`。用户遇到问题后重启应用，现场日志即丢失，"导出日志"命令（`commands/workspace.rs:195`）也随之失去排查价值。

**修复**：保留最近 3 份按启动时间命名的日志（`php-stack.1.log` 轮转），或至少改为 append + 启动时写入分隔线。约 20 行。

### R7.【P2·可靠】零散 panic 风险点 `[部分完成]`

- `backup_engine.rs:250`：`path.file_name().unwrap()` 对无名路径（如盘符根）会 panic；
- `logging.rs:27,62`：`Mutex::lock().unwrap()`，另一处持锁 panic 会毒化全局日志锁；
- `version_manifest.rs:46`：嵌入 JSON 解析 `expect`——这是编译期嵌入、可接受，但建议错误信息带上实际解析错误。

**修复**：均改为 `map_err` 降级处理，合计 15 行。

**实现偏差（2026-09-21）**：三处里已修两处（`backup_engine.rs` 的 `file_name().unwrap()`、`logging.rs` 的两处 `Mutex::lock().unwrap()`），都是在 R5 / R6 的改动中顺带完成的。`version_manifest.rs` 的 `include_str!` + `expect` 未动——它是编译期嵌入的固定资源，解析失败等于打包损坏，属于"启动即崩"的正确行为，改降级反而会掩盖问题；报告本身也标注"可接受，但建议错误信息带上实际解析错误"。

---

## 2. 架构设计

> 总体分层（commands → engine → docker）是清晰的，不需要推倒重来。以下是"简洁"维度的具体欠账。

### A1.【P0】路径解析逻辑重复三份，且生产模式写到 exe 同级目录 `[已完成]`

同一段"开发模式向上爬 4 层 / 生产模式取 exe 父目录"的逻辑出现在：
- `src-tauri/src/commands/mod.rs:17-45`（`get_project_root`）
- `src-tauri/src/commands/workspace.rs:196-214`（`export_logs` 里又抄了一遍）
- `src-tauri/src/lib.rs:13-25`（日志目录）

更关键的是**产品问题**：生产模式下 `workspace.json`、`.user_*.json`、`php-stack.log` 全部写在 exe 同级目录（`workspace_manager.rs:16-35`）。Windows 用户把应用装进 `Program Files` 后**无写权限**，功能直接失效；装到多台机器或绿色版多目录时配置互不隔离。

**修复**：
1. 三处合并为 `commands/mod.rs` 中唯一的 `paths` 模块（`project_root()` / `app_config_dir()` / `log_file()`）；
2. 用户级配置（workspace.json、.user_*.json、日志）迁移到 Tauri 官方 `app.path().app_data_dir()`（通过 `AppHandle` 传入，不再自己爬目录）；工作区内的文件（.env、services/、备份）保持原地不动——它们本来就是"工作区"的一部分。
3. 首次启动时检测旧位置文件并自动迁移 + 提示。

**实现偏差（2026-09-21）**：本轮只迁移了 `workspace.json` 与日志文件，`.user_mirror_config.json` / `.user_version_overrides.json` **保留在项目根**。原因：这两个文件被 `backup_engine` 按项目根打包、`restore_engine` 按项目根还原，且语义上随工作区走（换工作区就该换一套覆盖）——迁到 app_data_dir 会直接打断备份/恢复闭环，收益不抵风险。若后续要做，需连同备份/恢复管道一起改，届时应作为一个独立改动统一处理。

**实现方式**：`app_data_dir` 由 setup 注入一次后存入 `OnceLock`，而不是把 `AppHandle` 透传进 20 余处命令——后者会让所有命令签名膨胀，与"简单"原则冲突。未注入时回退旧逻辑，保证单元测试可用。

### A2.【P1】.env 键名解析依赖字符串前缀切片 `[已完成]`

`commands/env_config.rs` 的 `parse_env_to_services` 曾用 `&key[3..key.len()-8]`、`&key[6..key.len()-8]` 这类魔数切片反解 `PHP82_VERSION` / `NGINX127_HTTP_HOST_PORT`，且四个服务的解析代码高度雷同。新增服务类型或前缀规则变化时极易出错（NGINX 的"5 个字母 + 1 = 6"注释就是危险信号）。

**修复（2026-09-21）**：改为遍历 manifest 中各服务的 `service_dir` 生成 `{DIR}_VERSION` / `_HOST_PORT` / `_HTTP_HOST_PORT` 去匹配 env 键；四个服务循环抽成 `collect_services_from_manifest`。不再依赖魔数切片。

### A3.【P1】前端缺少统一的 API 层 `[已完成]`

`invoke` 曾直接散落在 8 个组件里，错误处理模式各不相同：EnvConfigPage 有 `formatErrorMessage`，其他页面直接 `showToast(e as string)`，Dashboard 只写日志不弹提示。`e as string` 假设后端总是返回字符串，遇到非字符串错误时会显示 `[object Object]`。

**修复（2026-09-21）**：新建 `src/api/`，按域封装（`client` / `backup` / `workspace` / `docker` / `envConfig` / `mirror`）。统一 `normalizeError` + `invokeCommand`（失败抛 string）；`PORT_CONFLICT:` 前缀助手供 App 启动流程使用。业务组件只调用 api 函数，不再直接 `invoke`。

### A4.【P2】镜像源三个模块职责重叠 `[未开始]`

`mirror_manager.rs`（379 行，预设+测试）、`mirror_config.rs`（342 行，标注"向后兼容"）、`mirror_config_manager.rs`（325 行，用户配置）三者共存，"向后兼容"模块中仍有活跃逻辑。建议做一次收束：明确一个对外门面（`mirror_config_manager`），把 `mirror_config.rs` 的兼容层真正冻结或删除。非紧急，但拖越久越难拆。

### A5.【P2】应用数据文件清单没有单一事实来源 `[未开始]`

当前磁盘写入物散布：项目根（.env、docker-compose.yml、services/、data/、logs/、.user_mirror_config.json、.user_version_overrides.json）、exe 目录（workspace.json、php-stack.log）。建议在 `doc/architecture/ARCHITECTURE.md` 增加一张"应用写入了什么、在哪、谁负责"的表，并作为 A1 重构的验收依据。

### A6.【P2】死代码与占位实现 `[已完成]`

- `commands/backup.rs:79-83`：`select_project_folder` 是空占位（返回 `Ok(None)`），却注册在 invoke_handler（lib.rs:73），前端从未调用 → 删除；
- `App.vue:53-61`：两个 `@ts-ignore` 的 computed（hasAnyContainers/hasStoppedContainers）→ 删除；
- `App.vue:152-181`：`openServiceConfig` 四个分支执行完全相同的 `replace('ps-','')` → 合并为一行；
- `App.vue:21-28`：`Container` 接口字段用了包装对象类型 `String` 而非 `string` → 修正。

**实现偏差（2026-09-21）**：容器相关部分已在 R4 提交中一并完成——`Container` 接口改为 `src/types/docker.ts` 的强类型定义（state 为 `ContainerState` 枚举），`@ts-ignore` 的两个 computed 已删除。`openServiceConfig` 四个分支合并、`App.vue` 的 `String` 残留未动，属纯样式清理。

---

## 3. 用户体验

> 界面风格统一、步骤式恢复向导、启动确认弹窗等都做得不错。以下是实际使用中会碰到的摩擦点。

### U1.【P0·功能缺陷】多个 PHP 服务时自定义扩展输入互相污染 `[已完成]`

`src/components/EnvConfigPage.vue:32` 只有一个 `customExtInput`，而 `syncCustomExtensions(phpIndex)`（:503-521）把这个共享输入合并进任意一个 PHP 服务。用户给 php82 填了 `xdebug`，切到 php85 的面板时同一个输入框内容还在，保存时会错误地合并进去。

**修复**：`customExtInput` 改为 `Record<number, string>`（项目里 `phpExtensionsPanelState` 已是同款模式，照抄即可）。约 10 行。**这是真实功能 bug，建议单独出一个 fix 提交。**

### U2.【P1】恢复结果只有"成功/失败"一句话，错误明细不可读 `[已完成]`

后端 `execute_restore` 把多行 errors 用 `\n` 拼进 Err（commands/backup.rs:70-74），前端 `showToast(e as string)` 弹一个 3 秒的 toast（RestorePage.vue:157）——用户既看不全也来不及看。而 `RestoreResult.restored_files` 明细后端算好了却完全没展示。

**修复**：`execute_restore` 改为始终返回 `RestoreResult`，前端在恢复步骤（RestorePage 第 4 步）渲染结果卡片：恢复文件列表 + 错误列表 + （配合 R2 的）回滚包路径提示。

### U3.【P1】Docker 不可用时仍每 5 秒全量轮询 `[已完成]`

`App.vue:339` 的 `setInterval` 永不清除（且组件卸载语义上也不需要，但 Docker 不可用时依旧每 5 秒 ping + 拉列表 + 写日志），Docker Desktop 未启动时日志面板会被 `dockerCheckFailed` 每隔 5 秒刷一条。

**修复**：连续失败 N 次后退避到 15-30 秒；Docker 恢复后恢复 5 秒。同时顺手把轮询间隔与"运行中"状态灯的 animate-pulse 区分开。

### U4.【P1】UI 日志与文件日志是两套东西，复制按钮语义混乱 `[已完成]`

日志面板显示的是内存中最近 50 条（`useToast.ts:20`，超过即丢），"复制"按钮复制的却是后端文件日志全文（`App.vue:391-398` → `export_logs`）。用户在界面上看到的错误未必复制得出来（50 条以前），复制出来的内容又和看到的对不上。另外面板没有"清空"按钮，长会话下满屏历史。

**修复**：复制按钮复制 UI 日志原文；面板加"清空"与"导出文件"两个动作（导出走后端文件日志）；文件日志本身配合 R6 做轮转。

### U5.【P2】i18n 收尾 `[部分完成]`

- 残留硬编码中文运行时文案：`useConfirmDialog.ts:25-27`（默认"确认操作/确认/取消"，英文界面下弹中文按钮）、`App.vue:465`（侧边栏折叠按钮 title）、`App.vue:639`（复制日志 title）；
- 后端用户可见日志（ui_log!）全部是中文 + emoji（如 env_config.rs 全文），英文界面下日志面板是中文。AGENTS.md 宣称"后端日志英文化"并未达成。
- **建议**：前端三处立即修；后端日志选择一个现实策略——要么承认"UI 日志面向中文用户、仅结构化字段英文"，写进文档；要么把 ui_log 的固定短语做成 key 传给前端翻译。不建议做完整后端 i18n（违背"简单"原则）。

**实现偏差（2026-09-21）**：只完成了 `App.vue` 复制日志按钮的硬编码 title（U4 提交中顺带改掉，新增 `dashboard.log.copyTip`）。`useConfirmDialog.ts` 的默认按钮文案与侧边栏折叠 title 未动，后端日志 i18n 按报告建议**明确不做**（违背"简单"原则）——这一策略尚未写进 AGENTS.md，建议后续补。

### U6.【P2】WorkspaceInitDialog 不适配亮色主题 `[未开始]`

对话框写死暗色（`bg-slate-900 border-slate-700`，无 `dark:` 前缀），亮色模式下是全应用唯一的突兀深色弹窗；确认后 `window.location.reload()` 整页刷新（WorkspaceInitDialog.vue:44），丢失所有未保存状态。建议：补 `dark:` 适配；reload 改为事件通知各页面重新加载工作区信息。

### U7.【P2】EnvConfigPage 内置的前端版本 fallback 列表会烂掉 `[未开始]`

`EnvConfigPage.vue:179-203` 在后端加载失败时使用一份硬编码版本清单，与 `services/version_manifest.json` 双源维护。建议失败时显示明确错误 + 重试按钮，而不是降级到一份必然过期的静态数据（备份一段"应急可用"的体验，代价是永久的双维护）。

### U8.【P2】清理调试输出与提高键盘可达性 `[未开始]`

- 22 处 `console.log`（主要集中在 EnvConfigPage 的加载流程）清理或降级为统一 debug 开关；
- 侧边栏导航与日志按钮是 `div @click`（App.vue:417-453），无 Tab 焦点、无 Enter 触发。桌面应用要求不高，但改成 `<button>` 零成本，建议顺手做。

---

## 4. 面向未来的扩展性

> 原则：只做"到时候能扩展"的准备，不做"提前建好的抽象"。

### E1.【P1】版本清单嵌入二进制，新增版本必须重新发版 `[未开始]`

`version_manifest.rs:44` 用 `include_str!` 把 `services/version_manifest.json` 编译进二进制。PHP 8.5 / MySQL 9.x 发布时，用户必须升级整个应用才能选新版本。而项目已有的 `.user_version_overrides.json` 机制只覆盖"改镜像 tag"，覆盖不了"新增条目"。

**建议**：启动时按 `app_data_dir/services/version_manifest.json`（若存在）→ 内置 fallback 的顺序加载，用户可下载新清单文件覆盖。**不**要做远程自动拉取（保持简单，避免新增网络依赖面）。约 30 行 + 文档。

### E2.【P2】新增服务类型（如 PostgreSQL）的改动点清单 `[未开始]`

当前需要同步修改：Rust 侧 `config_generator::ServiceType`、`version_manifest` 的 key 匹配、`load_existing_config` 解析分支、compose 模板；前端 `ServiceType` 联合类型、EnvConfigPage 四组几乎相同的服务数组与增删函数。建议：① 把 EnvConfigPage 中 PHP/MySQL/Redis/Nginx 四组 `ref` + `add/remove` 函数收敛为一份配置驱动的通用实现（当前就是复制粘贴的，约省 200 行）；② 在 `docs/architecture/EXTENSION_GUIDE.md` 补一份"新增服务类型 checklist"。

### E3.【P2】数据库备份/恢复（已知待完善项） `[未开始]`

AGENTS.md 已列为低优先级，建议维持，但给出实现边界以防过度设计：mysqldump 通过 bollard exec 在容器内执行、dump 到挂载目录（复用现有 volume 映射），**不要**在宿主机找 mysql client；恢复阶段只把 SQL 放入 `database/` 并提示用户确认后执行导入（与现有"恢复预览→确认"两段式一致）。进度事件复用 `backup-progress` 通道。

### E4.【P2】备份包版本兼容性检查缺失 `[未开始]`

manifest 有 `version: "1.0.0"` 字段（backup_manifest.rs），但 `RestoreEngine` 不校验它。未来格式升级后，旧版 app 打开新备份包会静默丢字段。**建议**：restore 预览时检查 `manifest.version` 是否在支持区间，不在则明确报"备份包版本过新/过旧，请升级应用"。约 10 行，现在做成本最低。

### E5.【P1】自动更新是面向用户的最大缺口 `[未开始]`

`tauri.conf.json` 无 updater 配置，release.yml 永远 `prerelease: true`，用户发现新版本的唯一方式是手动查看仓库。对一个分发给开发者的桌面工具，建议引入 `tauri-plugin-updater` + 签名密钥（GitHub Releases 作为源）。这是本报告唯一建议新增的重型功能，因为它直接决定用户能否留在新版本上。若暂不做，至少把 release 的 prerelease 改为正式版 + 在应用"关于"里放检查更新链接（shell open 即可，一天内完成）。

---

## 5. 工程质量与文档

### Q1.【P0】没有 CI 测试流水线 `[已完成]`

`.github/workflows/` 只有 release.yml（tag 触发）。当前 85 + 60 个测试全部依赖本地手动运行，"cargo test 通过"没有任何机器保证。**建议新增 `ci.yml`**：push/PR 触发，跑 `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`、`npm run test:run`、`npm run build`（Windows 单平台即可，成本可控）。这是所有其他质量工作的前提，约半天。

### Q2.【P0】文档与事实脱节（可信度问题） `[已完成]`

AI/新人依赖的 AGENTS.md 存在多处失实：
- "proptest 属性测试"——实际 0 处使用（依赖还挂在 Cargo.toml dev-deps 里，要么删依赖要么删描述）；
- "端口冲突检测与自动分配"（恢复 4.x）——恢复预览已检测宿主机端口占用并给出建议端口（2026-09-21）；恢复仍写入原配置，不自动改写端口（用户可在启动前手动修改）。
- "后端日志英文化"——未达成（见 U5）；
- 测试标签规范 `// Feature: ..., Property N: ...`——代码中未见执行。

**建议**：以本次评审为准修订 AGENTS.md（把未实现项移入"待完善"），CHANGELOG 补记 2026-09 的三笔修复（compose stop 保留容器、模板路径、i18n 文案）；`docs/README.md` 版本头已于 2026-09-20 doc/→docs/ 目录合并时更新至 v0.3.1。**规则化**：以后每个 fix 提交同时更新 CHANGELOG 的 Unreleased 段。

### Q3.【P1】LICENSE 缺失 `[已完成]`

README 底部链接 `[MIT](LICENSE)`，但文件不存在。要么补 MIT 全文（推荐，README 与 Cargo.toml license 字段都写了 MIT），要么改掉链接。

### Q4.【P1】无效测试要删掉 `[已完成]`

`commands/env_config.rs:854-928` 的三个"测试"创建临时文件后又删除，**没有任何断言**（注释自己承认"无法直接测试"）。它们制造了测试通过的假象。处理：把 `load_existing_config` 的解析主体抽成 `fn parse_env_to_config(env_map, manifest) -> Vec<ServiceEntry>` 纯函数，这三个用例改为对纯函数的真断言（数据都是现成的）。

### Q5.【P2】补充 lint 基建 `[未开始]`

无 eslint/prettier。建议加 `eslint` + `eslint-plugin-vue`（flat config）+ prettier，规则从宽（不做风格洁癖），仅拦截未用变量、明显的 hook 误用。Rust 侧 clippy 已清理过（f2c3 提交），纳入 CI 即可维持。

### Q6.【P2】CSP 为 null `[未开始]`

`tauri.conf.json:26`。桌面应用风险低于 Web，但前端只加载本地资源，配一条最小 CSP（`default-src 'self'` + style 允许 inline）成本半小时，堵住供应链类注入的口子。

### Q7.【P2】两个大组件的拆分（渐进式，不强制） `[未开始]`

`EnvConfigPage.vue`（1102 行）与 `App.vue`（812 行）已到可维护性边界。建议在下次功能迭代触碰它们时顺手拆：EnvConfigPage → 按服务类型拆 4 个子组件 + 1 个 composable；App.vue → Dashboard 视图 + 侧边栏 + 日志面板各自成组件，全局状态收敛到 `composables/useDocker.ts`（模式与 useToast 一致，不需要引 pinia）。**不建议**为此专门发起重构分支。

---

## 6. 落地路线图

> 排序原则：先堵可靠性漏洞，再修文档可信度（成本低收益高），最后做体验与扩展。每批完成后跑全量测试并记 CHANGELOG。

### 第一批：止血（0.5~1 天，建议单独分支 `fix/reliability-basics`）

| # | 事项 | 对应条目 | 预估 | 状态 |
|---|---|---|---|---|
| 1 | 修复 zip-slip + 恶意包回归测试 | R1 | 2h | ✅ 已完成 |
| 2 | 新增 ci.yml（fmt/clippy/test/vitest/build） | Q1 | 2h | ✅ 已完成 |
| 3 | 补 LICENSE 文件 | Q3 | 5min | ✅ 已完成 |
| 4 | 修订 AGENTS.md、CHANGELOG（docs/README.md 版本头已随目录合并更新） | Q2 | 1h | ✅ 已完成 |
| 5 | 删除死代码（select_project_folder、@ts-ignore computed、无效测试转为真测试） | A6、Q4 | 2h | ✅ 已完成 |
| 6 | 修复多 PHP 服务自定义扩展串扰 | U1 | 0.5h | ✅ 已完成 |

> 第一批 6 项已全部完成（2026-09-21 核对）。A6 中 `openServiceConfig` 四分支合并、App.vue 残留 `String` 未做，属纯样式清理，不影响功能。

### 第二批：可靠与体验（2~3 天）

| # | 事项 | 对应条目 | 预估 | 状态 |
|---|---|---|---|---|
| 1 | 恢复前自动备份（回滚包） | R2 | 2h | ✅ 已完成 |
| 2 | 恢复结果页展示明细 + 错误列表 | U2 | 2h | ✅ 已完成 |
| 3 | Docker 阻塞调用包 spawn_blocking | R3 | 2h | ✅ 已完成 |
| 4 | 容器状态枚举化（前后端契约） | R4 | 3h | ✅ 已完成 |
| 5 | 路径解析统一 + 用户配置迁至 app_data_dir | A1 | 4h | ✅ 已完成（有偏差，见 A1 条目） |
| 6 | 备份流式写入 | R5 | 3h | ✅ 已完成 |
| 7 | 日志轮转 + UI 日志复制/清空/导出 | R6、U4 | 2h | ✅ 已完成 |
| 8 | Docker 不可用轮询退避 | U3 | 1h | ✅ 已完成 |

> 第二批 8 项已全部完成（2026-09-21）。R3 中"顺手简化 `create_backup` 的 spawn_blocking + block_on 双层嵌套"**未做**——那是为了绕开 `BackupEngine` 返回非 Send future 的约束，改成单层 `tokio::spawn` 会直接编译失败，收益不抵风险。

### 第三批：扩展与打磨（按需排期）

| # | 事项 | 对应条目 | 状态 |
|---|---|---|---|
| 1 | 自动更新（tauri-plugin-updater 或最低配"检查更新"） | E5 | ⬜ 未开始 |
| 2 | 版本清单外部覆盖 | E1 | ⬜ 未开始 |
| 3 | manifest 版本兼容检查 | E4 | ⬜ 未开始 |
| 4 | .env 解析去魔数切片 | A2 | ✅ 已完成 |
| 5 | 前端 api 层 + EnvConfigPage 服务面板配置化 | A3、E2 | ✅ A3 已完成；E2 未开始 |
| 6 | i18n 收尾 + 主题适配收尾 + lint 基建 + CSP | U5、U6、Q5、Q6 | ⬜ 未开始（U5 部分完成） |
| 7 | mysqldump / SQL 导入（维持低优先级） | E3 | ⬜ 未开始 |

### 验收标准（每批公共）

- `cargo test`、`npm run test:run`、`npm run build` 全绿（由 CI 保证，不再口头保证）；
- 新增逻辑均有对应测试（R1 必须含恶意 ZIP 回归测试）；
- 不引入新依赖（E5 除外）；不改变现有配置文件的磁盘格式（A1 的迁移除外，且必须有旧路径迁移逻辑）；
- CHANGELOG Unreleased 段同步更新。

---

## 附录 A：证据索引（关键引用）

| 结论 | 证据位置 |
|---|---|
| zip-slip | `src-tauri/src/engine/restore_engine.rs:268-287` |
| 恢复覆盖无回滚 | `src-tauri/src/engine/restore_engine.rs:111-189` |
| 阻塞 async | `src-tauri/src/commands/env_config.rs:406,587,672` |
| 状态字符串契约 | `src-tauri/src/docker/manager.rs:57,122` ↔ `src/App.vue:79-83` |
| 备份全量入内存 | `src-tauri/src/engine/backup_engine.rs:123,256` |
| 日志 truncate | `src-tauri/src/logging.rs:19-24` |
| 路径逻辑三份 | `commands/mod.rs:17-45`、`commands/workspace.rs:196-214`、`lib.rs:13-25` |
| .env 魔数切片 | `commands/env_config.rs:139-263` |
| 扩展输入串扰 | `src/components/EnvConfigPage.vue:32,503-521` |
| 无效测试 | `src-tauri/src/commands/env_config.rs:854-928` |
| 死命令 | `src-tauri/src/commands/backup.rs:79-83`、`lib.rs:73` |
| 无 CI 测试 | `.github/workflows/` 仅 release.yml |
| proptest 0 使用 / 文档失实 | `grep proptest src-tauri/src` = 0；AGENTS.md「测试覆盖」节 |
| LICENSE 缺失 | 根目录无 LICENSE*，README.md:88 链接 MIT |
| 版本清单嵌入 | `src-tauri/src/engine/version_manifest.rs:44` |

## 附录 B：与"简单/简洁/实用/可靠"的对照自检

- **可靠**：当前主要缺口（第 1 节），第一批优先偿还；
- **简洁**：路径逻辑三份、.env 切片、四组复制粘贴的服务面板是最大三处技术债，均为"减法"重构，不引入新抽象；
- **实用**：U1-U4、E5 都是真实用户路径上的问题，修一个是一个；明确**不推荐**的事项：完整后端 i18n、远程版本清单拉取、数据库事务式恢复、pinia/vue-router 引入（当前规模不需要）；
- **简单**：所有建议的总量控制在"个人项目 + AI 协作"可维护的范围内，第三批全部为按需项，允许长期搁置。
