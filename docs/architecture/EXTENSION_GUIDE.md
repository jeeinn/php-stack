# PHP-Stack 扩展指南

> **版本**: v0.2.0 (2026-04-27)  
> ↩ [返回主架构文档](./ARCHITECTURE.md)

---

## 📋 目录

- [1. 添加新的服务版本](#1-添加新的服务版本)
- [2. 添加用户自定义标签](#2-添加用户自定义标签)
- [3. 添加新的后端 Command](#3-添加新的后端-command)

---

## 1. 添加新的服务版本

### 步骤 1: 编辑 `version_manifest.json`

在 `src-tauri/services/version_manifest.json` 中添加新条目。key 为 ID（与 `services/` 目录名一致），每条记录包含所有必需字段：

```json
{
  "mysql": {
    "mysql90": {
      "display_name": "MySQL 9.0",
      "image_tag": "mysql:9.0-innovation",
      "service_dir": "mysql90",
      "default_port": 3306,
      "show_port": true,
      "eol": false,
      "description": "MySQL 9.0 Innovation (创新版)"
    }
  }
}
```

**ID 命名规则**: `{service_name}{major}{minor}`，去掉版本号中的点号。例如：
- PHP 8.5 → `php85`
- MySQL 9.0 → `mysql90`
- Nginx 1.28 → `nginx128`
- Redis 8.2 → `redis82`

**字段说明**:

| 字段 | 类型 | 必填 | 说明 | 示例 |
|------|------|------|------|------|
| `display_name` | string | ✅ | 前端下拉列表显示名称 | `"MySQL 9.0"` |
| `image_tag` | string | ✅ | 完整 Docker 镜像名（可直接 `docker pull`） | `"mysql:9.0-innovation"` |
| `service_dir` | string | ✅ | 配置目录名（与 `services/` 下的子目录一致） | `"mysql90"` |
| `default_port` | number | ✅ | 默认端口号 | `3306` |
| `show_port` | boolean | ✅ | 是否在 UI 中显示端口配置 | `true` |
| `eol` | boolean | ✅ | 是否已停止维护 | `false` |
| `description` | string | ❌ | 版本描述 | `"MySQL 9.0 Innovation"` |

> **注意**: `env_prefix` 不需要单独配置，由 `service_dir.to_uppercase()` 自动推导。例如 `service_dir = "mysql90"` → `env_prefix = "MYSQL90"`。

### 步骤 2: 创建服务模板目录

```bash
mkdir -p src-tauri/services/mysql90
cp src-tauri/services/mysql84/mysql.cnf src-tauri/services/mysql90/
```

如果是 PHP 版本，还需要复制 Dockerfile：
```bash
mkdir -p src-tauri/services/php90
cp src-tauri/services/php85/Dockerfile src-tauri/services/php90/
cp src-tauri/services/php85/php.ini src-tauri/services/php90/
cp src-tauri/services/php85/php-fpm.conf src-tauri/services/php90/
```

### 步骤 3: 使清单生效

**方式 A（推荐，无需重新发版）**：将更新后的 `version_manifest.json` 放到应用数据目录：

| 平台 | 路径 |
|------|------|
| Windows | `%APPDATA%\com.php-stack.dev\services\version_manifest.json` |
| macOS | `~/Library/Application Support/com.php-stack.dev/services/version_manifest.json` |
| Linux | `~/.local/share/com.php-stack.dev/services/version_manifest.json` |

启动时优先加载该覆盖文件；解析失败则回退内置清单并写警告日志。

**方式 B**：改仓库内 `src-tauri/services/version_manifest.json` 后重新编译（`include_str!` 嵌入二进制）。

```bash
cd src-tauri && cargo build
```

---

## 1.1 不重新发版也能用上新版本

覆盖路径见上文「方式 A」。可用仓库内 `scripts/sync-version-manifest.mjs` 生成最新清单后拷贝到该路径。

---

## 2. 用户覆盖与自定义版本（L3）

与仓库侧 `sync-version-manifest`（L1）、运行时 `ConfigExtractor`（L2）正交：**用户数据只写在工作区 `.user-config/version_overrides.json`，不改内置/app_data 清单。**

每条记录 **`entry_kind` 必填**（不做旧格式兼容；缺字段则整文件视为空）：

| `entry_kind` | 来源 | 字段 | 语义 |
|---|---|---|---|
| `override` | 映射表「编辑」已有 ID | `image_tag` + 可选 `description` | 叠在清单基线上只换镜像 |
| `custom` | 映射表「新增」 | 完整 VersionEntry 字段 | 独立条目，可出现在环境配置下拉 |

### 方法 1: 通过 UI（软件版本映射）

- **编辑**：改已有版本的 Docker 镜像标签  
- **新增**：填写版本名 / ID / 镜像 / 配置目录 / 端口等；保存后映射表与环境配置下拉均可选用  

### 方法 2: 手动编辑 `.user-config/version_overrides.json`

```json
{
  "php": {
    "php82": {
      "entry_kind": "override",
      "image_tag": "php:8.2-fpm-alpine",
      "description": "使用 Alpine 版本减小体积"
    }
  },
  "redis": {
    "redis99": {
      "entry_kind": "custom",
      "display_name": "Redis 9.9",
      "image_tag": "redis:9.9-alpine",
      "service_dir": "redis82",
      "default_port": 6379,
      "show_port": true,
      "eol": false,
      "description": "自测版本"
    }
  }
}
```

**建议**：`custom` 的 `service_dir` 与版本 ID 一致（与 sync / 手工清单相同）；物理模板缺失时在 apply 阶段走 `resolve_template_dir` 回退或镜像 `docker create` 提取。

### 方法 3: 直接修改 `.env` 文件

```env
# PHP 8.2 使用 Alpine 版本
PHP82_VERSION=php:8.2-fpm-alpine

# MySQL 8.4 使用特定小版本
MYSQL84_VERSION=mysql:8.4.0

# 使用私有仓库镜像
PHP83_VERSION=registry.example.com/php:8.3-custom
```

> **注意**: 直接修改 `.env` 后，下次通过 GUI "应用配置"会覆盖这些手动修改。如需持久化自定义标签，请使用方法 1 或方法 2。

---

## 3. 添加新的后端 Command

### 步骤 1: 在 `commands/` 对应子模块中添加函数

根据命令的业务域，选择合适的子模块：
- `commands/docker.rs` — 容器操作相关
- `commands/env_config.rs` — 环境配置相关
- `commands/mirror.rs` — 镜像源管理相关
- `commands/backup.rs` — 备份恢复相关
- `commands/workspace.rs` — 工作区、版本管理、日志相关

```rust
#[tauri::command]
pub fn my_new_command(param: String) -> Result<String, String> {
    // 如需获取项目根目录，使用 super::get_project_root()
    let project_root = super::get_project_root()?;
    Ok(format!("Result: {}", param))
}
```

> 如果命令不属于任何现有子模块，可以新建子模块并在 `commands/mod.rs` 中声明和 re-export。

### 步骤 2: 在 `lib.rs` 中注册

```rust
.invoke_handler(tauri::generate_handler![
    // ... 其他命令
    commands::my_new_command,
])
```

### 步骤 3: 前端调用

```typescript
const result = await invoke('my_new_command', { param: 'test' });
```

### 步骤 4: 更新权限（如需要）

如果新命令需要特殊权限（如文件系统访问、剪贴板等），在 `src-tauri/capabilities/default.json` 中添加对应权限。

---

## 4. 新增服务类型（如 PostgreSQL）Checklist

当前服务类型写死为 PHP / MySQL / Redis / Nginx。新增一种类型需要同步改下列位置（按顺序勾选）：

### 后端
- [ ] `engine/config_generator.rs` — `ServiceType` 枚举 + compose / `.env` 生成分支
- [ ] `engine/version_manifest.rs` — `ServiceType` + `ManifestFile` 字段 + `from_json` 插入
- [ ] `services/version_manifest.json` — 新服务块与至少 1 个版本条目
- [ ] `services/<newtype>/` — Dockerfile / 配置模板（如有）
- [ ] `commands/env_config.rs` — `parse_env_to_services` 已由 manifest 驱动，一般只需补 `collect_services_from_manifest` 调用
- [ ] 集成测试：配置生成、备份 manifest 中的服务端口

### 前端
- [ ] `src/types/env-config.ts` — `ServiceType` / `ServiceTypeLower` / `VersionMappings`
- [ ] `EnvConfigPage.vue` — 服务列表 `ref`、增删、模板面板、版本下拉（目前四组结构相近，长期目标是配置驱动收敛，见 IMPROVEMENT_REPORT E2）
- [ ] `SoftwareSettings.vue` — `serviceLabels` 与 tab
- [ ] i18n：`envConfig.<service>.*`、`software.*` 中英 key

### 文档
- [ ] 更新本文件与 `ARCHITECTURE.md` 服务列表
- [ ] `AGENTS.md` 待完善项（如有）

> **不做**：远程自动拉取服务定义、通用插件系统（违背「简单」原则）。

---

↩ [返回主架构文档](./ARCHITECTURE.md)
