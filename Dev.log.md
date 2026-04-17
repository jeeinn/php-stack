# PHP-Stack 开发日志

## 2026-04-17 - 环境配置模块完善与 Docker 构建修复

### 📋 本次迭代目标
完善"环境配置"模块，实现配置文件生成、模板管理、Docker 启动等功能。

---

### 🐛 遇到的问题及解决方案

#### 1. 应用配置时 Vite 重启问题

**问题描述**：
点击"应用配置"后，Vite 开发服务器检测到 `.env` 文件变化并自动重启。

**根本原因**：
Vite 的文件监听器默认会监视项目根目录下的所有文件变化。

**解决方案**：
在 `vite.config.ts` 中添加忽略规则：
```typescript
server: {
  watch: {
    ignored: [
      '**/.env',
      '**/docker-compose.yml',
      '**/data/**',
      '**/logs/**',
      '**/services/**',
    ]
  }
}
```

**提交**: `c306ed8`

---

#### 2. 配置加载不正确

**问题描述**：
页面无法正确加载已有的配置信息。

**根本原因**：
后端解析逻辑期望 `PHP_VERSION` 格式，但实际 `.env` 使用 `PHP56_VERSION` 格式（带版本号）。

**解决方案**：
重构解析逻辑，遍历所有键值对并动态提取版本号：
```rust
for (key, value) in &env_map {
    if key.ends_with("_VERSION") && key.starts_with("PHP") {
        let ver_part = &key[3..key.len() - 8]; // 提取 "56" from "PHP56_VERSION"
        let port_key = format!("PHP{}_HOST_PORT", ver_part);
        // ...
    }
}
```

**提交**: `c306ed8`

---

#### 3. 模板路径计算错误

**问题描述**：
```
模板文件不存在: "E:\\study\\php-stack\\services\\php56/php-fpm.conf"
```

**根本原因**：
开发模式下路径计算错误，使用了 3 层 `.parent()` 导致从 `src-tauri/` 跳到了项目根目录。

**正确的路径层级**：
```
current_exe() -> src-tauri/target/debug/app.exe
  .parent()   -> target/debug/
  .parent()   -> src-tauri/  ✅ 只需要 2 层！
```

**解决方案**：
修正为 2 层 `.parent()`：
```rust
exe_dir
    .parent()       // target/debug/ -> target/
    .and_then(|p| p.parent())   // target/ -> src-tauri/
    .map(|p| p.join("services").join(template_name))
```

**提交**: `db0ef21`

---

#### 4. Nginx Dockerfile 缺失

**问题描述**：
```
target nginx: failed to solve: failed to read dockerfile: open Dockerfile: no such file or directory
```

**根本原因**：
`generate_service_dirs()` 函数在处理 Nginx 服务时，只复制了 `nginx.conf` 和 `conf.d/default.conf`，忘记复制 `Dockerfile`。

**解决方案**：
在 Nginx 分支中添加 Dockerfile 复制：
```rust
ServiceType::Nginx => {
    // Copy Dockerfile from template
    Self::copy_template_file(
        "nginx/Dockerfile",
        &service_dir.join("Dockerfile"),
    )?;
    // ...
}
```

**提交**: `5276c62`

---

#### 5. PHP 5.6/7.4 构建失败（Debian EOL）

**问题描述**：
```
RUN export option="--no-install-recommends -y" \
    && apt-get $option update \
    && apt-get $option install tzdata curl
exit code: 100
```

**根本原因**：
- `php:5.6-fpm` 基于 Debian 8 (Jessie) - 2018 年已 EOL
- `php:7.4-fpm` 基于 Debian 10 (Buster) - 2024 年已 EOL
- EOL 版本的 apt 源已被移除，`apt-get update` 必然失败

**解决方案**：
改用 Alpine 基础镜像：
```dockerfile
# 之前
FROM php:5.6-fpm
RUN apt-get update && apt-get install tzdata curl

# 之后
FROM php:5.6-fpm-alpine
RUN apk --no-cache add tzdata curl
```

**优势**：
- 镜像更小（~50MB vs ~450MB）
- 持续维护和支持
- 更快的构建速度

**提交**: `d15e7b6`

---

#### 6. PHP 扩展安装参数格式错误

**问题描述**：
```
parsePackageName(): invalid package name "mysqli,mbstring,curl,redis"
invalid package name/package file "mysqli,mbstring,curl,redis"
install failed
```

**根本原因**：
- `.env` 文件中扩展名使用逗号分隔：`pdo_mysql,mbstring,gd,curl,redis`
- `install-php-extensions` 工具要求空格分隔：`pdo_mysql mbstring gd curl redis`

**解决方案**：
在所有 PHP Dockerfile 中使用 `tr` 命令转换：
```dockerfile
# 之前
RUN install-php-extensions $PHP_EXTENSIONS

# 之后
RUN install-php-extensions $(echo $PHP_EXTENSIONS | tr ',' ' ')
```

**影响文件**：
php56, php74, php80, php81, php82, php83, php84, php85 的 Dockerfile

**提交**: `cf59861`

---

#### 7. 容器名称冲突

**问题描述**：
```
Container ps-mysql  Error response from daemon: Conflict. 
The container name "/ps-mysql" is already in use by container "1d900317ed81..."
```

**发生场景**：
1. 首次启动失败，留下 `Created` 状态的容器
2. 修复问题后再次启动，Docker Compose 尝试创建同名容器 → 冲突

**解决方案**：
启动前先清理旧容器：
```rust
// 第一步：清理旧容器
emit_log("🧹 清理旧容器...");
Command::new("docker")
    .args(&["compose", "down", "--remove-orphans"])
    .current_dir(&project_root)
    .output()?;

// 第二步：启动新容器
Command::new("docker")
    .args(&["compose", "up", "-d"])
    .current_dir(&project_root)
    .output()?;
```

**优势**：
- 支持重复点击"一键启动"
- 自动处理异常情况
- 确保干净的启动环境

**提交**: `894a949`

---

#### 8. Tauri v2 emit 方法编译错误

**问题描述**：
```
error[E0599]: no method named `emit` found for struct `AppHandle`
```

**根本原因**：
Tauri v2 中 `emit` 方法需要通过 `Emitter` trait 引入。

**解决方案**：
在使用前导入 trait：
```rust
use tauri::Emitter;

let _ = app_handle.emit("env-log", msg);
```

**提交**: `7cc8afb`

---

### ✨ 新增功能

#### 1. 全链路日志系统

**后端日志**（终端输出）：
- `[APPLY_CONFIG]` 前缀：应用配置过程
- `[START_ENV]` 前缀：环境启动过程
- 包含详细的项目路径、执行命令、Docker Compose 输出

**前端日志**（UI 显示）：
- 通过 `listen('env-log')` 监听后端事件
- 实时显示在底部"实时日志"面板
- 带时间戳，最多保留 50 条

**示例**：
```
[START_ENV] 🚀 开始启动环境...
[START_ENV] 📁 项目根目录: "E:\study\php-stack"
[START_ENV] 🧹 清理旧容器...
[START_ENV] ✅ 旧容器已清理
[START_ENV] 🔧 执行: docker compose up -d
[START_ENV] ⏳ 首次启动可能需要几分钟...
[START_ENV] 📤 Docker Compose 输出:
[START_ENV]    Building php56...
[START_ENV] ✅ 环境启动成功！
```

**提交**: `7cc8afb`, `6f2a3f4`

---

#### 2. PHP 版本完整支持（5.6 - 8.5）

**前端版本列表**：
```typescript
const phpVersions = ['5.6', '7.4', '8.0', '8.1', '8.2', '8.3', '8.4', '8.5'];
```

**版本映射关系**：
| PHP 版本 | 模板目录 | Docker 基础镜像 | 状态 |
|---------|---------|----------------|------|
| 5.6 | php56 | php:5.6-fpm-alpine | Alpine |
| 7.4 | php74 | php:7.4-fpm-alpine | Alpine |
| 8.0 | php80 | php:8.0-fpm | Debian |
| 8.1 | php81 | php:8.1-fpm | Debian |
| 8.2 | php82 | php:8.2-fpm | Debian |
| 8.3 | php83 | php:8.3-fpm | Debian |
| 8.4 | php84 | php:8.4-fpm | Debian |
| 8.5 | php85 | php:8.5-fpm | Debian |

**新增模板**：
- php81, php83, php84, php85 的完整配置（Dockerfile, php.ini, php-fpm.conf）

**提交**: `dd65856`

---

#### 3. 服务配置模板系统

**模板目录结构**：
```
src-tauri/services/
├── mysql/mysql.cnf
├── nginx/
│   ├── Dockerfile
│   ├── nginx.conf
│   └── conf.d/default.conf
├── php56/ (Alpine)
├── php74/ (Alpine)
├── php80/ (Debian)
├── php81/ (Debian)
├── php82/ (Debian)
├── php83/ (Debian)
├── php84/ (Debian)
├── php85/ (Debian)
└── redis/redis.conf
```

**打包配置**：
在 `tauri.conf.json` 中添加：
```json
{
  "bundle": {
    "resources": [
      "services/**/*"
    ]
  }
}
```

**Git 忽略规则**：
```gitignore
# Ignore runtime services directory (but keep src-tauri/services templates)
services/
!src-tauri/services/
```

**提交**: `3e46302`

---

### 📊 关键决策

#### 1. Docker 启动方式：Command vs bollard

**决策**：保持使用 `Command::new("docker")`

**理由**：
- ✅ bollard 不支持 `docker compose`（只提供底层 Docker API）
- ✅ 需要手动解析 docker-compose.yml，代码复杂度高
- ✅ 当前方案简单直接，日志功能已完善
- ✅ Docker Compose 智能缓存机制足够优秀

#### 2. 构建参数：--build vs 无参数

**决策**：不使用 `--build` 参数

**理由**：
- ✅ Docker Compose 会自动检测 Dockerfile 和配置文件变化
- ✅ 充分利用 layer caching，未变化的层直接复用
- ✅ 简化用户界面，无需选择 build/no-build
- ✅ 修改配置后"应用配置"再"启动"即可触发重建

#### 3. 启动前清理策略

**决策**：每次启动前执行 `docker compose down --remove-orphans`

**理由**：
- ✅ 避免容器名称冲突
- ✅ 支持重复点击"一键启动"
- ✅ 自动处理上次启动失败的残留容器
- ✅ `--remove-orphans` 只清理当前项目的容器，安全

---

### 🔧 技术要点

#### 1. Tauri v2 事件系统

**后端发送**：
```rust
use tauri::Emitter;
app_handle.emit("env-log", msg)?;
```

**前端接收**：
```typescript
import { listen } from '@tauri-apps/api/event';
listen('env-log', (event) => {
  const msg = event.payload as string;
  addLog(msg);
});
```

#### 2. Rust 路径处理

**开发模式路径计算**：
```rust
// current_exe() -> src-tauri/target/debug/app.exe
exe_dir
    .parent()       // target/debug/
    .and_then(|p| p.parent())   // src-tauri/
    .map(|p| p.join("services").join(template_name))
```

#### 3. Docker 镜像选型

**PHP 5.6/7.4**：使用 Alpine（Debian 已 EOL）
**PHP 8.0+**：使用 Debian（仍在支持期）

**判断依据**：
- 查看官方 Docker Hub 镜像说明
- 检查 Debian/Alpine 版本支持周期
- 参考 dnmp 项目的最佳实践

---

### 📝 待优化项

1. **Docker Compose version 属性警告**
   ```
   the attribute `version` is obsolete, it will be ignored
   ```
   - 可以从生成的 docker-compose.yml 中移除 `version: "3"`

2. **首次构建时间较长**
   - PHP 扩展安装耗时 2-5 分钟
   - 可以考虑预构建常用版本的镜像

3. **错误恢复机制**
   - 当前启动失败后需要手动清理
   - 可以添加"强制重置"按钮

---

### 🎯 经验总结

1. **路径问题最难排查** - 添加调试日志是关键
2. **Docker 基础镜像选型很重要** - 注意 EOL 状态
3. **参数格式要匹配工具要求** - 仔细阅读文档
4. **幂等性设计提升用户体验** - 允许重复操作
5. **日志系统是调试利器** - 前后端都要有

---

**记录时间**: 2026-04-17  
**涉及 Commit**: c306ed8, 3e46302, 3c1f52f, 7301a9c, db0ef21, 5276c62, d15e7b6, cf59861, 7cc8afb, 6f2a3f4, dd65856, 91686a9, 351d5d0, 894a949
