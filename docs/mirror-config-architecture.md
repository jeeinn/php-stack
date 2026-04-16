# 镜像源配置架构说明

**更新日期**: 2026-04-16  
**版本**: V1.0

---

## 📋 架构概述

PHP-Stack 项目中有**两类不同的镜像源配置**，需要明确区分：

### 1️⃣ Docker Daemon 镜像源（Docker Hub 加速）

**作用**: 加速从 Docker Hub 拉取镜像（如 `php:8.2-fpm`、`mysql:8.0`）

**配置文件**: `/etc/docker/daemon.json` (Linux) 或 `%USERPROFILE%\.docker\daemon.json` (Windows)

**实现模块**: `src-tauri/src/docker/mirror.rs`

**关键函数**:
```rust
pub fn set_docker_mirror(mirror_url: &str) -> Result<(), Box<dyn std::error::Error>>
```

**使用场景**:
- Docker Desktop 设置 → Docker Engine → registry-mirrors
- 或通过 Tauri 命令 `set_docker_mirror()` 修改

**示例配置**:
```json
{
  "registry-mirrors": [
    "https://docker.mirrors.ustc.edu.cn",
    "https://mirror.ccs.tencentyun.com"
  ]
}
```

---

### 2️⃣ 容器内依赖镜像源（APT/Composer/PyPI/NPM）

**作用**: 加速容器内的依赖安装命令（如 `apt-get install`、`composer install`、`pip install`）

**配置文件**: `.env`（项目根目录）

**实现模块**: `src-tauri/src/engine/mirror_config.rs`

**关键结构体**:
```rust
pub struct MirrorConfig {
    pub apt_mirror: MirrorSource,      // APT 软件源
    pub composer_mirror: MirrorSource, // Composer PHP 包管理器
    pub pypi_mirror: MirrorSource,     // PyPI Python 包管理器
    pub npm_mirror: MirrorSource,      // NPM JavaScript 包管理器
    pub http_proxy: Option<String>,    // HTTP 代理（可选）
    pub https_proxy: Option<String>,   // HTTPS 代理（可选）
    pub no_proxy: Option<String>,      // 不代理的地址
}
```

**使用场景**:
- 构建自定义 PHP 镜像时，在 Dockerfile 中注入镜像源配置
- 加速 `docker-php-ext-install`、`pecl install` 等命令

**示例配置** (`.env`):
```bash
APT_MIRROR=aliyun
COMPOSER_MIRROR=aliyun
PYPI_MIRROR=aliyun
NPM_MIRROR=taobao
HTTP_PROXY=http://127.0.0.1:7890
```

**生成的 Dockerfile 片段**:
```dockerfile
# 配置 APT 镜像源
RUN sed -i 's|deb.debian.org/debian|http://mirrors.aliyun.com/debian/|g' /etc/apt/sources.list

# 配置 Composer 镜像源
RUN composer config -g repo.packagist composer https://mirrors.aliyun.com/composer/

# 配置 PyPI 镜像源
RUN pip config set global.index-url https://mirrors.aliyun.com/pypi/simple/
```

---

## 🔄 工作流程对比

### Docker Daemon 镜像源流程

```
用户操作
  ↓
Docker Desktop 设置 / set_docker_mirror()
  ↓
修改 daemon.json
  ↓
重启 Docker Desktop
  ↓
生效：所有 docker pull 命令加速
```

### 容器内依赖镜像源流程

```
用户操作
  ↓
编辑 .env 文件 / update_mirror_config()
  ↓
MirrorConfig::load_from_env()
  ↓
generate_dockerfile() 生成 Dockerfile
  ↓
docker build 时注入镜像源配置
  ↓
生效：容器内的 apt-get/composer/pip 加速
```

---

## 📊 功能对比表

| 特性 | Docker Daemon 镜像源 | 容器内依赖镜像源 |
|------|---------------------|-----------------|
| **作用范围** | Docker 引擎级别 | 容器内部 |
| **影响命令** | `docker pull` | `apt-get`, `composer`, `pip`, `npm` |
| **配置文件** | `daemon.json` | `.env` |
| **实现模块** | `docker/mirror.rs` | `engine/mirror_config.rs` |
| **是否需要重启** | ✅ 需要重启 Docker | ❌ 无需重启 |
| **支持的镜像源** | Docker Registry | APT/Composer/PyPI/NPM |
| **配置方式** | JSON 格式 | KEY=VALUE 格式 |
| **Tauri 命令** | `set_docker_mirror()` | `get_mirror_config()`, `update_mirror_config()` |

---

## 🎯 最佳实践

### 推荐配置组合

#### 中国大陆用户

**Docker Daemon** (`daemon.json`):
```json
{
  "registry-mirrors": [
    "https://docker.mirrors.ustc.edu.cn"
  ]
}
```

**容器内依赖** (`.env`):
```bash
APT_MIRROR=aliyun
COMPOSER_MIRROR=aliyun
PYPI_MIRROR=aliyun
NPM_MIRROR=taobao
```

#### 海外用户

**Docker Daemon**: 无需配置（使用默认 Docker Hub）

**容器内依赖**: 使用默认值或根据所在地区选择最近的镜像源

---

## 🔧 Tauri 命令清单

### Docker Daemon 相关

```rust
// src-tauri/src/docker/mirror.rs
#[tauri::command]
pub fn set_docker_mirror(url: String) -> Result<(), String>
```

**前端调用**:
```typescript
await invoke('set_docker_mirror', { 
  url: 'https://docker.mirrors.ustc.edu.cn' 
});
```

---

### 容器内依赖相关

```rust
// src-tauri/src/engine/mirror_config.rs
#[tauri::command]
pub async fn get_mirror_config() -> Result<MirrorConfig, String>

#[tauri::command]
pub async fn update_mirror_config(config: MirrorConfig) -> Result<(), String>

#[tauri::command]
pub async fn test_mirror_connection(source: MirrorSource) -> Result<bool, String>
```

**前端调用**:
```typescript
// 获取当前配置
const config = await invoke('get_mirror_config');

// 更新配置
await invoke('update_mirror_config', {
  config: {
    apt_mirror: 'aliyun',
    composer_mirror: 'aliyun',
    pypi_mirror: 'aliyun',
    npm_mirror: 'taobao',
  }
});

// 测试连接
const isConnected = await invoke('test_mirror_connection', {
  source: 'aliyun'
});
```

---

## ⚠️ 注意事项

### 1. 不要混淆两种配置

❌ **错误做法**:
```bash
# .env 中配置 DOCKER_REGISTRY_MIRROR（无效！）
DOCKER_REGISTRY_MIRROR=aliyun
```

✅ **正确做法**:
```bash
# .env 只配置容器内依赖镜像源
APT_MIRROR=aliyun
COMPOSER_MIRROR=aliyun

# Docker Daemon 镜像源通过 Docker Desktop 设置
```

### 2. 修改 Docker Daemon 配置后需重启

```bash
# Linux
sudo systemctl restart docker

# Windows/Mac
# 重启 Docker Desktop 应用
```

### 3. 容器内镜像源配置即时生效

- 修改 `.env` 后，下次构建镜像时自动生效
- 无需重启 Docker
- 已运行的容器不受影响

### 4. 代理配置的优先级

如果同时配置了镜像源和代理：
- **代理优先**: HTTP_PROXY 会覆盖镜像源配置
- **建议**: 二选一，避免冲突

---

## 📝 迁移指南

### 从旧版 `get_php_mirror_commands()` 迁移

**旧代码** (`docker/mirror.rs`):
```rust
// ❌ 已废弃
let commands = MirrorManager::get_php_mirror_commands("aliyun");
// 输出: ["sed -i ...", "composer config ..."]
```

**新代码** (`engine/mirror_config.rs`):
```rust
// ✅ 推荐
let config = MirrorConfig::load_from_env()?;
let snippet = config.to_dockerfile_snippet();
// 输出: "# 配置 APT 镜像源\nRUN sed -i ...\n\n# 配置 Composer...\n"
```

**优势**:
- ✅ 配置集中管理（`.env` 文件）
- ✅ 支持更多镜像源（阿里云、清华、中科大等）
- ✅ 更灵活（可单独配置 APT/Composer/PyPI/NPM）
- ✅ 易于维护（无需硬编码 URL）

---

## 🔍 故障排查

### 问题 1: Docker pull 速度慢

**检查**:
```bash
# 查看 Docker Daemon 配置
cat ~/.docker/daemon.json  # Linux/Mac
type %USERPROFILE%\.docker\daemon.json  # Windows
```

**解决**:
- 配置 registry-mirrors
- 重启 Docker Desktop

### 问题 2: apt-get/composer 速度慢

**检查**:
```bash
# 查看 .env 配置
cat .env
```

**解决**:
- 配置 APT_MIRROR、COMPOSER_MIRROR
- 重新构建镜像

### 问题 3: 镜像源连接失败

**测试**:
```typescript
// 前端调用
const isConnected = await invoke('test_mirror_connection', {
  source: 'aliyun'
});

if (!isConnected) {
  alert('镜像源不可用，请切换其他源');
}
```

**解决**:
- 尝试其他镜像源（清华、中科大）
- 检查网络连接
- 配置 HTTP 代理

---

## 📚 相关文档

- [Docker 官方文档 - Registry Mirrors](https://docs.docker.com/registry/recipes/mirror/)
- [阿里云镜像加速器](https://cr.console.aliyun.com/cn-hangzhou/instances/mirrors)
- [清华大学开源软件镜像站](https://mirrors.tuna.tsinghua.edu.cn/)
- [中科大开源软件镜像站](https://mirrors.ustc.edu.cn/)

---

**最后更新**: 2026-04-16  
**维护者**: PHP-Stack Team
