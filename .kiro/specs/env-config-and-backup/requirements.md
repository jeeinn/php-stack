# 需求文档：环境可视化配置与备份恢复

## 简介

本功能为 php-stack 项目（基于 Tauri v2 的 PHP 开发环境管理工具）增加三大核心能力：**可视化环境配置**、**统一镜像源管理**、**备份与恢复**。参考 dnmp 项目的最佳实践（`.env` → `docker-compose.yml` 的配置驱动模式、`services/`/`data/`/`logs/` 目录结构），在 php-stack 的 GUI 中实现等价的配置管理体验，并提供一键迁移恢复能力。

## 术语表

- **Config_Generator**：Rust 后端模块，负责根据用户 GUI 输入生成 `.env` 文件和 `docker-compose.yml` 文件
- **Mirror_Manager**：Rust 后端模块，负责统一管理 Docker 镜像拉取源、容器内 APT/Composer/NPM/PyPI 镜像源
- **Backup_Engine**：Rust 后端模块，负责将当前环境配置、服务配置文件、数据库数据、项目文件打包为 ZIP 备份包
- **Restore_Engine**：Rust 后端模块，负责解析备份包清单并在新机器上还原完整环境
- **Env_File**：项目根目录下的 `.env` 文件，存储所有服务版本、端口、扩展、路径、镜像源等配置变量
- **Compose_File**：项目根目录下的 `docker-compose.yml` 文件，通过 `${VAR}` 语法引用 Env_File 中的变量
- **Backup_Manifest**：备份 ZIP 包内的 `manifest.json` 文件，记录备份元数据（版本、时间戳、服务列表、文件清单）
- **Mirror_Preset**：预定义的镜像源组合方案（如"阿里云全套"、"清华大学全套"），一键切换所有镜像源
- **Service_Config_Dir**：`services/` 目录，存放各服务的 Dockerfile 和配置文件（如 `services/php82/php.ini`）
- **Data_Dir**：`data/` 目录，存放各服务的持久化数据（如 `data/mysql/`、`data/redis/`）
- **Logs_Dir**：`logs/` 目录，存放各服务的日志文件（如 `logs/nginx/`、`logs/php82/`）
- **PS_Container**：以 `ps-` 为前缀命名的 Docker 容器，是 php-stack 管理的容器标识约定

## 需求

### 需求 1：可视化 .env 配置生成

**用户故事：** 作为开发者，我希望通过 GUI 界面配置 PHP 版本、MySQL 版本、端口、扩展等参数，以便自动生成 `.env` 文件和 `docker-compose.yml` 文件，无需手动编辑。

#### 验收标准

1. WHEN 用户在 GUI 中选择服务类型（PHP、MySQL、Redis、Nginx）及其版本号，THE Config_Generator SHALL 将用户选择写入 Env_File，格式为 `SERVICE_VERSION=x.y.z`（如 `PHP82_VERSION=8.2.27`）。
2. WHEN 用户在 GUI 中为某个服务配置宿主机端口，THE Config_Generator SHALL 将端口写入 Env_File，格式为 `SERVICE_HOST_PORT=端口号`（如 `MYSQL_HOST_PORT=3306`）。
3. WHEN 用户在 GUI 中为 PHP 版本选择扩展列表，THE Config_Generator SHALL 将扩展列表写入 Env_File，格式为 `PHP{VER}_EXTENSIONS=ext1,ext2,ext3`（如 `PHP82_EXTENSIONS=pdo_mysql,mysqli,gd,curl,opcache`）。
4. WHEN 用户在 GUI 中配置项目源码目录路径，THE Config_Generator SHALL 将路径写入 Env_File，格式为 `SOURCE_DIR=路径`。
5. WHEN 用户点击"生成配置"按钮，THE Config_Generator SHALL 读取 Env_File 中的所有变量，生成使用 `${VAR}` 插值语法的 Compose_File，其中每个服务的 image、ports、volumes、environment 均引用 Env_File 中的对应变量。
6. WHEN 用户配置的两个服务使用相同的宿主机端口，THE Config_Generator SHALL 在用户提交前显示端口冲突错误提示，并阻止生成配置。
7. THE Config_Generator SHALL 按照 dnmp 最佳实践生成目录结构，包含 `services/`（各服务 Dockerfile 和配置文件）、`data/`（持久化数据卷）、`logs/`（服务日志）三个顶层目录。
8. WHEN 用户启用多个 PHP 版本（如 PHP 7.4 和 PHP 8.2），THE Config_Generator SHALL 在 Compose_File 中为每个 PHP 版本生成独立的服务定义，容器名遵循 `ps-php-{版本}` 格式。
9. WHEN 用户修改已有配置并重新生成，THE Config_Generator SHALL 保留 Env_File 中用户手动添加的自定义变量（非 Config_Generator 管理的变量）。

### 需求 2：统一镜像源管理

**用户故事：** 作为中国大陆的开发者，我希望在一个界面中统一管理所有镜像源（Docker 拉取源、APT 源、Composer 源、NPM 源），以便一键切换加速方案，避免逐个配置。

#### 验收标准

1. THE Mirror_Manager SHALL 在 GUI 中展示以下镜像源类别的当前配置状态：Docker Registry 镜像源、APT/Debian 软件源、PHP Composer 镜像源、NPM 镜像源。
2. WHEN 用户选择一个 Mirror_Preset（如"阿里云全套"），THE Mirror_Manager SHALL 将该预设对应的所有镜像源 URL 同时写入 Env_File 和 Docker Daemon 配置。
3. WHEN 用户为单个镜像源类别选择不同的源（如 APT 用清华、Composer 用阿里云），THE Mirror_Manager SHALL 独立保存每个类别的配置，不影响其他类别。
4. WHEN 用户点击某个镜像源的"测试连接"按钮，THE Mirror_Manager SHALL 向该镜像源 URL 发送 HTTP 请求，并在 3 秒内返回连接成功或失败的结果。
5. WHEN 用户修改镜像源配置后点击"应用"，THE Mirror_Manager SHALL 将新配置写入 Env_File，使后续的 Docker 镜像构建和容器内包管理命令使用新的镜像源。
6. THE Mirror_Manager SHALL 提供以下 Mirror_Preset 选项：阿里云全套、清华大学全套、腾讯云全套、中科大全套、官方默认源。
7. IF 用户选择的镜像源连接测试失败，THEN THE Mirror_Manager SHALL 显示失败原因并建议用户选择其他可用源，但不阻止用户保存配置。

### 需求 3：环境备份

**用户故事：** 作为开发者，我希望一键导出当前开发环境的完整备份（包括配置、数据库、项目文件），以便在新机器上快速恢复相同的开发环境。

#### 验收标准

1. WHEN 用户点击"创建备份"按钮，THE Backup_Engine SHALL 生成一个 ZIP 格式的备份包，包含以下内容：Env_File、Compose_File、Service_Config_Dir 中的所有配置文件、Backup_Manifest。
2. WHEN 用户在备份选项中勾选"包含数据库数据"，THE Backup_Engine SHALL 对每个运行中的 MySQL PS_Container 执行 `mysqldump` 命令，将导出的 SQL 文件存入备份包的 `database/` 目录。
3. WHEN 用户在备份选项中勾选"包含项目文件"并指定 glob 模式，THE Backup_Engine SHALL 按照 glob 模式匹配 SOURCE_DIR 下的文件，将匹配的文件存入备份包的 `projects/` 目录。
4. THE Backup_Engine SHALL 在 Backup_Manifest 中记录以下元数据：备份版本号、创建时间戳（ISO 8601 格式）、php-stack 应用版本、操作系统信息、服务列表（每个服务的名称、镜像、版本、端口映射）、备份选项、文件清单及各文件的 SHA256 校验和。
5. WHILE 备份过程执行中，THE Backup_Engine SHALL 通过 Tauri 事件机制向前端发送进度更新，包含当前步骤名称和完成百分比。
6. IF 备份过程中某个 MySQL 容器的 `mysqldump` 命令执行失败，THEN THE Backup_Engine SHALL 记录错误信息到 Backup_Manifest 的 `errors` 字段，继续备份其他内容，并在备份完成后向用户展示部分失败的警告。
7. WHEN 用户在备份选项中勾选"包含 Nginx 虚拟主机配置"，THE Backup_Engine SHALL 将 `services/nginx/conf.d/` 目录下的所有 `.conf` 文件存入备份包的 `vhosts/` 目录。
8. THE Backup_Engine SHALL 在备份包中包含 `logs/` 目录下最近 7 天的日志文件（可选，默认不包含）。

### 需求 4：环境恢复

**用户故事：** 作为开发者，我希望在新机器上导入备份包后一键恢复开发环境，以便快速开始工作而无需重新配置。

#### 验收标准

1. WHEN 用户选择一个备份 ZIP 文件并点击"开始恢复"，THE Restore_Engine SHALL 解压备份包并解析 Backup_Manifest，在 GUI 中展示备份内容摘要（服务列表、数据库列表、项目文件数量、备份时间）。
2. WHEN 用户确认恢复操作，THE Restore_Engine SHALL 将 Env_File 和 Compose_File 还原到项目根目录。
3. WHEN 用户确认恢复操作，THE Restore_Engine SHALL 将 Service_Config_Dir 中的配置文件还原到对应的 `services/` 子目录。
4. WHEN 备份包中包含数据库 SQL 文件，THE Restore_Engine SHALL 在对应的 MySQL PS_Container 启动并就绪后，执行 SQL 文件以恢复数据库数据。
5. WHEN 备份包中包含项目文件，THE Restore_Engine SHALL 将项目文件还原到 Env_File 中 `SOURCE_DIR` 指定的目录。
6. WHEN 恢复过程中检测到备份包中的服务端口与当前机器上已占用的端口冲突，THE Restore_Engine SHALL 在 GUI 中展示冲突详情，并提供自动分配可用端口的选项。
7. WHILE 恢复过程执行中，THE Restore_Engine SHALL 通过 Tauri 事件机制向前端发送进度更新，包含当前步骤名称和完成百分比。
8. IF 恢复过程中某个步骤失败（如镜像拉取失败、SQL 执行失败），THEN THE Restore_Engine SHALL 记录错误信息，跳过该步骤继续执行后续步骤，并在恢复完成后向用户展示失败步骤的详细错误信息和手动修复建议。
9. WHEN 备份包中包含 Nginx 虚拟主机配置文件，THE Restore_Engine SHALL 将 `.conf` 文件还原到 `services/nginx/conf.d/` 目录。
10. THE Restore_Engine SHALL 在恢复前验证 Backup_Manifest 中记录的文件 SHA256 校验和，确保备份包未被篡改或损坏。

### 需求 5：Env_File 解析器与格式化器

**用户故事：** 作为系统内部模块，Config_Generator 需要可靠地读取和写入 `.env` 文件，以便在不丢失注释和自定义内容的前提下更新配置变量。

#### 验收标准

1. WHEN 给定一个合法的 Env_File 内容字符串，THE Env_Parser SHALL 将其解析为键值对集合，正确处理带引号的值（单引号和双引号）、行内注释、空行和纯注释行。
2. THE Env_Formatter SHALL 将键值对集合格式化为 Env_File 内容字符串，保留原始文件中的注释行和空行位置。
3. FOR ALL 合法的 Env_File 内容，解析后再格式化（parse → format → parse）SHALL 产生与首次解析等价的键值对集合（往返一致性）。
4. IF 给定的 Env_File 内容包含无法解析的行（如缺少 `=` 的非注释行），THEN THE Env_Parser SHALL 返回包含行号和内容的描述性错误信息。

### 需求 6：Backup_Manifest 序列化与反序列化

**用户故事：** 作为系统内部模块，Backup_Engine 和 Restore_Engine 需要可靠地生成和解析 `manifest.json` 文件，以便准确记录和还原备份元数据。

#### 验收标准

1. THE Manifest_Serializer SHALL 将 Backup_Manifest 结构体序列化为格式化的 JSON 字符串（缩进 2 空格）。
2. WHEN 给定一个合法的 Backup_Manifest JSON 字符串，THE Manifest_Deserializer SHALL 将其反序列化为 Backup_Manifest 结构体。
3. FOR ALL 合法的 Backup_Manifest 结构体，序列化后再反序列化（serialize → deserialize → serialize）SHALL 产生与首次序列化等价的 JSON 输出（往返一致性）。
4. IF 给定的 JSON 字符串缺少必需字段（version、timestamp、services），THEN THE Manifest_Deserializer SHALL 返回描述性错误信息，指明缺少的字段名称。
