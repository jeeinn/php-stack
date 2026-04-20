# 配置文件路径架构说明

## 📋 概述

本文档说明 PHP-Stack 项目中各类配置文件的存放位置策略，区分开发环境和生产环境。

---

## 🗂️ 配置文件分类

### 1. 用户数据文件（运行时生成）

这些文件在运行时动态生成，与 `.env` 同级，**不应纳入版本控制**。

| 文件 | 位置 | 说明 | .gitignore |
|------|------|------|-----------|
| `.env` | 项目根目录 | 环境变量配置 | ✅ |
| `docker-compose.yml` | 项目根目录 | Docker Compose 配置 | ✅ |
| `.user_version_overrides.json` | 项目根目录 | 用户版本覆盖配置 | ✅ |
| `.npmrc` | 项目根目录 | NPM 镜像配置 | ✅ |
| `data/` | 项目根目录 | 数据库数据目录 | ✅ |
| `logs/` | 项目根目录 | 日志文件目录 | ✅ |
| `services/` | 项目根目录 | 运行时服务配置 | ✅ |

### 2. 模板文件（版本控制）

这些是配置模板，存放在 `src-tauri/services/`，**需要纳入版本控制**。

| 文件/目录 | 位置 | 说明 | .gitignore |
|----------|------|------|-----------|
| `version_manifest.json` | `src-tauri/services/` | 版本清单模板 | ❌ |
| `php*/Dockerfile` | `src-tauri/services/php*/` | PHP 构建模板 | ❌ |
| `mysql*/mysql.cnf` | `src-tauri/services/mysql*/` | MySQL 配置模板 | ❌ |
| `redis*/redis.conf` | `src-tauri/services/redis*/` | Redis 配置模板 | ❌ |
| `nginx*/nginx.conf` | `src-tauri/services/nginx*/` | Nginx 配置模板 | ❌ |

---

## 🔄 开发与生产环境对比

### 开发环境（Development）

```
项目根目录/
├── src-tauri/              # Rust 后端代码
│   ├── services/           # 配置模板（版本控制）
│   │   ├── version_manifest.json
│   │   ├── php84/
│   │   ├── mysql84/
│   │   └── ...
│   └── src/
├── .env                    # 生成的环境变量 ✨
├── docker-compose.yml      # 生成的 Compose 配置 ✨
├── .user_version_overrides.json  # 用户覆盖配置 ✨
├── data/                   # 运行时数据 ✨
├── logs/                   # 运行时日志 ✨
└── services/               # 运行时服务配置 ✨
    ├── php84/
    ├── mysql84/
    └── ...
```

**关键路径逻辑** (`commands.rs::get_project_root()`):
```rust
if cfg!(debug_assertions) {
    // 开发模式：项目根目录（src-tauri 的父目录）
    current_exe()
        → target/debug/app.exe
        → target/debug/
        → target/
        → src-tauri/
        → 项目根目录/  ← 返回这里
}
```

### 生产环境（Production）

```
安装目录/
├── php-stack.exe          # 可执行文件
├── services/              # 配置模板（打包时复制）
│   ├── version_manifest.json
│   ├── php84/
│   ├── mysql84/
│   └── ...
├── .env                   # 生成的环境变量 ✨
├── docker-compose.yml     # 生成的 Compose 配置 ✨
├── .user_version_overrides.json  # 用户覆盖配置 ✨
├── data/                  # 运行时数据 ✨
├── logs/                  # 运行时日志 ✨
└── services/              # 运行时服务配置 ✨
    ├── php84/
    ├── mysql84/
    └── ...
```

**关键路径逻辑** (`commands.rs::get_project_root()`):
```rust
else {
    // 生产模式：可执行文件所在目录
    current_exe()
        → 安装目录/php-stack.exe
        → 安装目录/  ← 返回这里
}
```

---

## 🎯 设计原则

### 原则 1: 用户数据与模板分离

```
✅ 正确：
- 模板：src-tauri/services/ （版本控制）
- 数据：项目根目录/ （不版本控制）

❌ 错误：
- 用户数据放在 src-tauri/ 下
- 模板放在项目根目录
```

### 原则 2: 统一的用户数据位置

所有运行时生成的文件都在**同一层级**：
- `.env`
- `docker-compose.yml`
- `.user_version_overrides.json`
- `data/`
- `logs/`
- `services/` (运行时)

**优势**：
- 📁 结构清晰，易于理解
- 🔍 便于备份和迁移
- 🗑️ 清理时只需删除整个目录
- 💾 方便打包为用户数据存档

### 原则 3: 环境自适应

通过 `cfg!(debug_assertions)` 自动区分环境：
- **开发环境**：从 `target/debug/` 向上导航到项目根目录
- **生产环境**：直接使用可执行文件所在目录

**优势**：
- ✅ 无需手动配置路径
- ✅ 开发和生产行为一致
- ✅ 支持任意安装位置

---

## 📝 历史变更

### 2026-04-20: 修正用户覆盖配置文件路径

**之前**：
```
src-tauri/.user_version_overrides.json  ❌
```

**问题**：
- 与 `.env`、`docker-compose.yml` 不在同一目录
- 开发环境和生产环境路径不一致
- 不符合用户数据与模板分离原则

**之后**：
```
.user_version_overrides.json  ✅
```

**改进**：
- ✅ 与 `.env` 同级，结构清晰
- ✅ 开发和生产环境使用相同相对路径
- ✅ 符合用户数据文件管理规范
- ✅ 便于备份和迁移

**涉及修改**：
1. `user_override_manager.rs`: 移除硬编码的 `src-tauri/` 前缀
2. `.gitignore`: 更新忽略规则
3. 文档: 更新所有路径引用

---

## 🔍 常见问题

### Q1: 为什么 `.user_version_overrides.json` 不放在 `src-tauri/` 下？

**A**: 
- `src-tauri/` 是**源代码目录**，包含模板和代码
- 用户覆盖配置是**运行时数据**，应该与 `.env` 同级
- 便于用户备份和迁移（只需复制项目根目录的用户数据文件）

### Q2: 开发环境和生产环境的文件位置会不同吗？

**A**: 
- **不会**！通过 `get_project_root()` 函数自动适配
- 开发环境：项目根目录（Git 仓库根目录）
- 生产环境：可执行文件所在目录（安装目录）
- 相对路径始终是：`.user_version_overrides.json`

### Q3: 如果我想备份用户配置，需要备份哪些文件？

**A**: 备份项目根目录下的这些文件/目录：
```bash
# PowerShell
Copy-Item .env, docker-compose.yml, .user_version_overrides.json, data/, logs/ -Destination backup/ -Recurse
```

### Q4: 如何清理所有运行时生成的文件？

**A**: 删除以下文件和目录（保留 `src-tauri/`）：
```bash
# PowerShell
Remove-Item .env, docker-compose.yml, .user_version_overrides.json, .npmrc -ErrorAction SilentlyContinue
Remove-Item data/, logs/, services/ -Recurse -ErrorAction SilentlyContinue
```

---

## 📚 相关文档

- [USER_OVERRIDE_GUIDE.md](./USER_OVERRIDE_GUIDE.md) - 用户版本覆盖功能使用指南
- [VERIFY_USER_OVERRIDE.md](./VERIFY_USER_OVERRIDE.md) - 配置验证指南
- [ARCHITECTURE.md](../ARCHITECTURE.md) - 系统架构文档

---

**最后更新**: 2026-04-20  
**适用版本**: PHP-Stack V2.1+
