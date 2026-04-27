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

### 步骤 3: 重新编译

```bash
cd src-tauri && cargo build
```

**无需修改任何 Rust 代码！** manifest 通过 `include_str!` 嵌入到二进制中，编译时自动加载。

---

## 2. 添加用户自定义标签

### 方法 1: 通过 UI（软件设置页面）

1. 打开"软件设置"页面
2. 在版本列表中找到目标版本
3. 点击"编辑"按钮
4. 在 `image_tag` 输入框中输入完整镜像名（如 `php:8.2-fpm-alpine`）
5. 可选填写备注说明
6. 点击"保存"

### 方法 2: 手动编辑 `.user_version_overrides.json`

在项目根目录（与 `.env` 同级）创建或编辑 `.user_version_overrides.json`：

```json
{
  "php": {
    "php82": {
      "image_tag": "php:8.2-fpm-alpine",
      "description": "使用 Alpine 版本减小体积"
    }
  },
  "mysql": {
    "mysql84": {
      "image_tag": "registry.company.com/mysql:8.4-custom",
      "description": "使用公司内部镜像"
    }
  }
}
```

**key 说明**: 外层 key 为服务类型（`php`/`mysql`/`redis`/`nginx`），内层 key 为 manifest ID（如 `php82`）。

**覆盖规则**: 用户覆盖仅替换 `image_tag`（和可选的 `description`），其他字段（`display_name`、`service_dir`、`default_port`、`show_port`、`eol`）保持 manifest 默认值不变。

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

### 步骤 1: 在 `commands.rs` 中添加函数

```rust
#[tauri::command]
pub fn my_new_command(param: String) -> Result<String, String> {
    // 实现逻辑
    Ok(format!("Result: {}", param))
}
```

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

↩ [返回主架构文档](./ARCHITECTURE.md)
