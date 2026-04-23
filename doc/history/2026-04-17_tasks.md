# 实现计划：环境可视化配置与备份恢复

## 概述

本计划将设计文档中定义的 6 个 Rust 模块和 4 个 Vue 组件拆解为可增量执行的编码任务。实现顺序遵循模块依赖关系：先实现基础设施模块（env_parser、backup_manifest），再实现依赖它们的高层模块（config_generator、mirror_manager、backup_engine、restore_engine），最后添加 Tauri 命令层和 Vue 前端组件。

所有 Rust 属性测试使用 `proptest` crate，标签格式：`// Feature: env-config-and-backup, Property N: {property_text}`。

## 任务

- [x] 1. 添加 proptest 开发依赖
  - 在 `src-tauri/Cargo.toml` 的 `[dev-dependencies]` 中添加 `proptest = "1"` 和 `sha2 = "0.10"`（用于 Property 8 的 SHA256 测试）
  - 确认 `cargo check` 通过
  - _需求: 全局（测试基础设施）_

- [ ] 2. 实现 env_parser.rs — Env_File 解析器与格式化器
  - [x] 2.1 创建 `src-tauri/src/engine/env_parser.rs` 并在 `mod.rs` 中注册
    - 实现 `EnvLine` 枚举（Empty、Comment、KeyValue）
    - 实现 `EnvFile` 结构体及其方法：`parse()`、`format()`、`to_map()`、`set()`、`get()`、`remove()`
    - 实现 `EnvParseError` 结构体，包含 `line_number`、`content`、`message` 字段
    - 解析逻辑需正确处理：带引号的值（单引号/双引号）、行内注释（`#`）、空行、纯注释行
    - _需求: 5.1, 5.2, 5.3, 5.4_

  - [ ]* 2.2 编写属性测试：Env_File 往返一致性
    - **Property 9: Env_File 往返一致性**
    - 使用 proptest 生成随机合法 `.env` 内容（键值对、带引号值、行内注释、空行、纯注释行）
    - 验证 `parse → format → parse` 后键值对集合与首次 `parse` 等价
    - **验证: 需求 5.1, 5.2, 5.3**

  - [ ]* 2.3 编写属性测试：Env_File 解析错误报告
    - **Property 10: Env_File 解析错误报告**
    - 使用 proptest 生成包含无法解析行（缺少 `=` 的非注释、非空行）的 `.env` 内容
    - 验证 `parse` 返回错误，且错误信息包含行号和原始内容
    - **验证: 需求 5.4**

- [x] 3. 检查点 — 确保所有测试通过
  - 运行 `cargo test`，确保所有测试通过，如有问题请询问用户。

- [ ] 4. 实现 backup_manifest.rs — Backup_Manifest 数据模型与序列化
  - [x] 4.1 创建 `src-tauri/src/engine/backup_manifest.rs` 并在 `mod.rs` 中注册
    - 定义 `BackupManifest` 结构体（version、timestamp、app_version、os_info、services、options、files、errors）
    - 定义 `ManifestService` 结构体（name、image、version、ports）
    - 定义 `BackupOptions` 结构体（include_database、include_projects、project_patterns、include_vhosts、include_logs）
    - 所有结构体派生 `Serialize`、`Deserialize`、`Debug`、`Clone`
    - 实现序列化方法：使用 `serde_json::to_string_pretty`（缩进 2 空格）
    - 实现反序列化方法：使用 `serde_json::from_str`，缺少必需字段时返回描述性错误
    - _需求: 6.1, 6.2, 6.3, 6.4_

  - [ ]* 4.2 编写属性测试：Backup_Manifest 往返一致性
    - **Property 11: Backup_Manifest 往返一致性**
    - 使用 proptest 生成随机 `BackupManifest` 结构体
    - 验证 `serialize → deserialize → serialize` 后 JSON 字符串与首次 `serialize` 完全相同
    - **验证: 需求 6.1, 6.2, 6.3**

  - [ ]* 4.3 编写属性测试：Manifest 反序列化错误报告
    - **Property 12: Manifest 反序列化错误报告**
    - 使用 proptest 生成缺少必需字段（version、timestamp、services 中任一个）的 JSON 字符串
    - 验证 `deserialize` 返回错误，且错误信息指明缺少的字段名称
    - **验证: 需求 6.4**

- [x] 5. 检查点 — 确保所有测试通过
  - 运行 `cargo test`，确保所有测试通过，如有问题请询问用户。

- [ ] 6. 实现 config_generator.rs — 可视化配置生成器
  - [x] 6.1 创建 `src-tauri/src/engine/config_generator.rs` 并在 `mod.rs` 中注册
    - 定义 `EnvConfig`、`ServiceEntry`、`ServiceType` 数据结构（派生 Serialize/Deserialize）
    - 实现 `ConfigGenerator::validate()` — 端口冲突检测
    - 实现 `ConfigGenerator::generate_env()` — 根据 `EnvConfig` 生成 `EnvFile`，保留已有自定义变量
    - 实现 `ConfigGenerator::generate_compose()` — 生成使用 `${VAR}` 插值的 docker-compose.yml 内容
    - 实现 `ConfigGenerator::generate_service_dirs()` — 创建 `services/`、`data/`、`logs/` 目录结构
    - 实现 `ConfigGenerator::apply()` — 写入 .env、docker-compose.yml、创建目录
    - _需求: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 1.9_

  - [ ]* 6.2 编写属性测试：配置生成正确性
    - **Property 1: 配置生成正确性**
    - 使用 proptest 生成随机合法 `EnvConfig`（任意数量服务、版本号、端口、扩展列表）
    - 验证 `generate_env` 输出的 `EnvFile` 包含与输入一一对应的键值对：`SERVICE_VERSION`、`SERVICE_HOST_PORT`、`PHP{VER}_EXTENSIONS`、`SOURCE_DIR`
    - **验证: 需求 1.1, 1.2, 1.3, 1.4**

  - [ ]* 6.3 编写属性测试：Compose 文件使用变量插值
    - **Property 2: Compose 变量插值**
    - 使用 proptest 生成随机合法 `EnvConfig`
    - 验证 `generate_compose` 输出中每个服务的 `image`、`ports`、`volumes` 使用 `${VAR}` 语法引用 `.env` 变量，而非硬编码值
    - **验证: 需求 1.5**

  - [ ]* 6.4 编写属性测试：端口冲突检测
    - **Property 3: 端口冲突检测**
    - 使用 proptest 生成包含两个或以上服务且存在相同宿主机端口的 `EnvConfig`
    - 验证 `validate` 返回错误，且错误信息包含冲突端口号和涉及的服务名称
    - **验证: 需求 1.6**

  - [ ]* 6.5 编写属性测试：目录结构生成
    - **Property 4: 目录结构生成**
    - 使用 proptest 生成随机合法 `EnvConfig`
    - 验证 `generate_service_dirs` 在目标路径下创建 `services/`、`data/`、`logs/` 三个顶层目录，且每个启用的服务在 `services/` 下有对应子目录
    - **验证: 需求 1.7**

  - [ ]* 6.6 编写属性测试：多 PHP 版本独立服务
    - **Property 5: 多 PHP 版本独立服务**
    - 使用 proptest 生成包含 N 个不同 PHP 版本的 `EnvConfig`（N ≥ 1）
    - 验证 `generate_compose` 输出包含恰好 N 个 PHP 服务定义，每个 `container_name` 遵循 `ps-php-{版本}` 格式且互不相同
    - **验证: 需求 1.8**

  - [ ]* 6.7 编写属性测试：自定义变量保留
    - **Property 6: 自定义变量保留**
    - 使用 proptest 生成已有 `EnvFile`（含 Config_Generator 管理的变量和用户自定义变量）+ 新的 `EnvConfig`
    - 验证 `generate_env` 输出保留所有用户自定义变量（非 Config_Generator 管理的键值对），且值不变
    - **验证: 需求 1.9**

- [x] 7. 检查点 — 确保所有测试通过
  - 运行 `cargo test`，确保所有测试通过，如有问题请询问用户。

- [ ] 8. 实现 mirror_manager.rs — 统一镜像源管理器
  - [x] 8.1 创建 `src-tauri/src/engine/mirror_manager.rs` 并在 `mod.rs` 中注册
    - 定义 `MirrorPreset` 结构体（name、docker_registry、apt、composer、npm）
    - 实现 `MirrorManager::get_presets()` — 返回 5 个预设方案（阿里云、清华、腾讯云、中科大、官方默认）
    - 实现 `MirrorManager::apply_preset()` — 同时更新 .env 和 Docker Daemon 配置
    - 实现 `MirrorManager::update_single()` — 独立更新单个镜像源类别
    - 实现 `MirrorManager::test_connection()` — 3 秒超时的 HTTP 连接测试
    - 实现 `MirrorManager::get_current_status()` — 获取当前所有镜像源状态
    - 使用 `env_parser.rs` 读写 `.env` 文件
    - _需求: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7_

  - [ ]* 8.2 编写属性测试：镜像源类别独立性
    - **Property 7: 镜像源类别独立性**
    - 使用 proptest 生成随机类别和随机源
    - 验证更新单个类别后，其他类别的值与更新前完全相同
    - **验证: 需求 2.3**

- [ ] 9. 实现 backup_engine.rs — 增强备份引擎
  - [x] 9.1 创建 `src-tauri/src/engine/backup_engine.rs` 并在 `mod.rs` 中注册
    - 实现 `BackupEngine::create_backup()` — 执行完整备份流程
      - 打包 .env、docker-compose.yml、services/ 配置文件
      - 可选：mysqldump 数据库导出（通过 Docker exec）
      - 可选：按 glob 模式匹配项目文件
      - 可选：Nginx vhost 配置文件
      - 可选：最近 7 天日志文件
    - 实现 `BackupEngine::compute_sha256()` — 计算文件 SHA256 校验和
    - 生成 `BackupManifest` 并写入 ZIP 包的 `manifest.json`
    - 通过 `app_handle.emit()` 发送 `BackupProgress` 进度事件
    - 单步失败记录到 `manifest.errors`，不中断整体流程
    - _需求: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8_

  - [ ]* 9.2 编写属性测试：SHA256 完整性验证
    - **Property 8: SHA256 完整性验证**
    - 使用 proptest 生成随机文件内容和对应的 SHA256
    - 验证：文件内容被修改后 `verify_integrity` 返回失败；文件未修改时返回成功
    - **验证: 需求 4.10**

- [x] 10. 检查点 — 确保所有测试通过
  - 运行 `cargo test`，确保所有测试通过，如有问题请询问用户。

- [ ] 11. 实现 restore_engine.rs — 恢复引擎
  - [x] 11.1 创建 `src-tauri/src/engine/restore_engine.rs` 并在 `mod.rs` 中注册
    - 定义 `RestorePreview`、`PortConflict`、`RestoreProgress` 数据结构
    - 实现 `RestoreEngine::preview()` — 解析备份包，返回预览信息（manifest、端口冲突、缺失镜像）
    - 实现 `RestoreEngine::verify_integrity()` — SHA256 校验备份包完整性
    - 实现 `RestoreEngine::restore()` — 执行恢复流程
      - 还原 .env 和 docker-compose.yml
      - 还原 services/ 配置文件
      - 还原 Nginx vhost 配置
      - 还原项目文件到 SOURCE_DIR
      - 执行数据库 SQL 恢复
    - 实现 `RestoreEngine::detect_port_conflicts()` — 端口冲突检测与自动分配
    - 通过 `app_handle.emit()` 发送 `RestoreProgress` 进度事件
    - 单步失败跳过并记录，完成后汇总展示
    - _需求: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7, 4.8, 4.9, 4.10_

  - [ ]* 11.2 编写单元测试：恢复引擎核心逻辑
    - 测试端口冲突检测与自动分配算法
    - 测试 manifest 解析与预览信息生成
    - 测试文件还原路径映射
    - _需求: 4.1, 4.6_

- [x] 12. 添加 Tauri 命令层
  - [x] 12.1 在 `src-tauri/src/commands.rs` 中添加配置生成命令
    - `validate_env_config` — 验证 EnvConfig
    - `generate_env_config` — 生成 .env 内容预览
    - `preview_compose` — 预览 docker-compose.yml
    - `apply_env_config` — 应用配置（写入文件、创建目录）
    - _需求: 1.1-1.9_

  - [x] 12.2 在 `src-tauri/src/commands.rs` 中添加镜像源管理命令
    - `get_mirror_presets` — 获取预设方案列表
    - `apply_mirror_preset` — 应用预设方案
    - `update_single_mirror` — 更新单个镜像源
    - `test_mirror` — 测试镜像源连接
    - `get_mirror_status` — 获取当前镜像源状态
    - _需求: 2.1-2.7_

  - [x] 12.3 在 `src-tauri/src/commands.rs` 中添加备份命令
    - `create_backup` — 创建备份（接收 `BackupOptions` 和 `app_handle`）
    - _需求: 3.1-3.8_

  - [x] 12.4 在 `src-tauri/src/commands.rs` 中添加恢复命令
    - `preview_restore` — 预览备份包内容
    - `verify_backup` — 验证备份包完整性
    - `execute_restore` — 执行恢复（接收端口覆盖映射和 `app_handle`）
    - _需求: 4.1-4.10_

  - [x] 12.5 在 `src-tauri/src/lib.rs` 中注册所有新命令到 `invoke_handler`
    - 将 12.1-12.4 中定义的所有命令添加到 `tauri::generate_handler![]` 宏
    - _需求: 全局_

- [x] 13. 检查点 — 确保所有测试通过且编译成功
  - 运行 `cargo test` 和 `cargo build`，确保所有测试通过且编译无错误，如有问题请询问用户。

- [x] 14. 实现 Vue 前端组件
  - [x] 14.1 定义前端 TypeScript 类型
    - 在 `src/types/` 目录下创建类型定义文件（或在各组件中内联定义）
    - 定义 `EnvConfig`、`ServiceEntry`、`BackupOptions`、`BackupManifest`、`RestorePreview`、`PortConflict`、`BackupProgress`、`MirrorPreset` 等接口
    - 类型需与 Rust 后端的 Serialize/Deserialize 结构体对应
    - _需求: 全局_

  - [x] 14.2 实现 `src/components/EnvConfigPage.vue` — 可视化 .env 配置页面
    - 服务选择表单：PHP（支持多版本）、MySQL、Redis、Nginx 的版本选择
    - 端口配置输入框，实时端口冲突检测提示
    - PHP 扩展列表多选
    - 项目源码目录路径输入
    - "预览配置"按钮 — 调用 `generate_env_config` 和 `preview_compose`
    - "应用配置"按钮 — 调用 `apply_env_config`
    - 使用 Tailwind CSS v4 样式，遵循项目现有 UI 风格
    - _需求: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 1.9_

  - [x] 14.3 实现 `src/components/MirrorPanel.vue` — 统一镜像源管理面板
    - 展示 4 个镜像源类别的当前配置状态（Docker Registry、APT、Composer、NPM）
    - 预设方案下拉选择（阿里云、清华、腾讯云、中科大、官方默认）
    - 单个类别独立配置
    - 每个类别的"测试连接"按钮，显示连接结果
    - "应用"按钮 — 调用 `apply_mirror_preset` 或 `update_single_mirror`
    - 连接失败时显示警告但不阻止保存
    - _需求: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7_

  - [x] 14.4 实现 `src/components/BackupPage.vue` — 备份页面
    - 备份选项勾选：包含数据库、包含项目文件（glob 模式输入）、包含 vhost 配置、包含日志
    - 保存路径选择（使用 Tauri dialog 插件）
    - "创建备份"按钮 — 调用 `create_backup`
    - 进度条 — 监听 Tauri 事件 `backup-progress`，显示当前步骤和百分比
    - 备份完成后展示结果摘要（成功/部分失败警告）
    - _需求: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8_

  - [x] 14.5 实现 `src/components/RestorePage.vue` — 恢复页面
    - ZIP 文件选择（使用 Tauri dialog 插件）
    - "预览"按钮 — 调用 `preview_restore`，展示备份内容摘要
    - 完整性验证 — 调用 `verify_backup`，显示校验结果
    - 端口冲突展示与自动分配选项
    - "开始恢复"按钮 — 调用 `execute_restore`
    - 进度条 — 监听 Tauri 事件 `restore-progress`，显示当前步骤和百分比
    - 恢复完成后展示结果摘要（成功/部分失败详情和手动修复建议）
    - _需求: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7, 4.8, 4.9, 4.10_

  - [x] 14.6 在 `src/App.vue` 中集成新页面的导航入口
    - 在侧边栏导航中添加"环境配置"、"镜像源"、"备份"、"恢复"四个导航项
    - 配置路由或条件渲染逻辑
    - _需求: 全局_

- [x] 15. 最终检查点 — 确保所有测试通过
  - 运行 `cargo test` 确保 Rust 后端所有测试通过
  - 运行前端构建命令确保无编译错误
  - 确保所有测试通过，如有问题请询问用户。

## 备注

- 标记 `*` 的任务为可选任务，可跳过以加速 MVP 开发
- 每个任务引用了具体的需求编号，确保可追溯性
- 检查点任务确保增量验证，及早发现问题
- 属性测试验证设计文档中定义的 12 个正确性属性
- 单元测试覆盖属性测试不适合的场景（如端口分配算法、文件路径映射）
