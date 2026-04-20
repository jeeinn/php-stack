# 镜像源配置使用指南

## 📋 目录
- [概述](#概述)
- [配置项说明](#配置项说明)
- [作用时机](#作用时机)
- [配置步骤](#配置步骤)
- [常见问题](#常见问题)

---

## 概述

PHP-Stack 提供了完整的镜像源加速配置，包括：
- 🐳 **Docker Registry** - Docker 镜像加速
- 📦 **APT/Debian** - Linux 软件包加速
- 🎵 **Composer** - PHP 依赖包加速
- 📗 **NPM** - Node.js 模块加速
- 🐙 **GitHub Proxy** - GitHub 资源下载加速

这些配置可以显著提升国内用户的开发体验，特别是在首次构建 Docker 镜像时。

---

## 配置项说明

### 1. Docker Registry Mirror
**环境变量**: `DOCKER_REGISTRY_MIRROR`  
**配置文件**: Docker Desktop 全局设置

**推荐镜像**：
- 阿里云: `https://registry.cn-hangzhou.aliyuncs.com`
- 腾讯云: `https://mirror.ccs.tencentyun.com`
- 中科大: `https://docker.mirrors.ustc.edu.cn`

**配置方法**：
1. 打开 Docker Desktop
2. Settings → Docker Engine
3. 添加配置：
```json
{
  "registry-mirrors": [
    "https://registry.cn-hangzhou.aliyuncs.com"
  ]
}
```
4. 点击 "Apply & Restart"

> ⚠️ **注意**: Docker Registry 镜像需要在 Docker Desktop 中全局配置，无法通过项目级别控制。

---

### 2. APT/Debian Mirror
**环境变量**: `APT_MIRROR`  
**配置文件**: `.env` → `docker-compose.yml` → Dockerfile

**推荐镜像**：
- 阿里云: `https://mirrors.aliyun.com/debian`
- 清华大学: `https://mirrors.tuna.tsinghua.edu.cn/debian`
- 腾讯云: `https://mirrors.cloud.tencent.com/debian`

**工作原理**：
```dockerfile
# Dockerfile 中自动执行
RUN sed -i "s|deb.debian.org|${DEBIAN_MIRROR_DOMAIN}|g" /etc/apt/sources.list
RUN apt-get update && apt-get install ...
```

**生效时机**：
- 构建 Docker 镜像时
- 执行 `docker compose up --build` 时

---

### 3. Composer Mirror
**环境变量**: `COMPOSER_MIRROR`  
**配置文件**: `.env` + `.npmrc` + Dockerfile

**推荐镜像**：
- 阿里云: `https://mirrors.aliyun.com/composer/`
- 腾讯云: `https://mirrors.cloud.tencent.com/composer/`
- 华为云: `https://repo.huaweicloud.com/repository/php/`

**工作原理**：
```dockerfile
# Dockerfile 中自动配置
RUN composer config -g repo.packagist composer $COMPOSER_MIRROR
```

**生效范围**：
1. **容器内**: Docker 镜像构建时配置的 Composer
2. **宿主机**: 用户在自己的项目中执行 `composer install` 时

---

### 4. NPM Mirror
**环境变量**: `NPM_MIRROR`  
**配置文件**: `.env` + `.npmrc` + Dockerfile

**推荐镜像**：
- 淘宝 (npmmirror): `https://registry.npmmirror.com`
- 腾讯云: `https://mirrors.cloud.tencent.com/npm/`
- 华为云: `https://repo.huaweicloud.com/repository/npm/`

**工作原理**：
```dockerfile
# Dockerfile 中自动配置
RUN npm config set registry $NPM_REGISTRY
```

```ini
# .npmrc 文件（项目根目录）
registry=https://registry.npmmirror.com
```

**生效范围**：
1. **容器内**: Docker 镜像中的 npm
2. **宿主机**: 用户在项目根目录执行 `npm install` 时（通过 `.npmrc`）

---

### 5. GitHub Proxy
**环境变量**: `GITHUB_PROXY`  
**配置文件**: `.env` → Dockerfile

**推荐代理**：
- ghproxy.com: `https://ghproxy.com`
- github.moeyy.xyz: `https://github.moeyy.xyz`
- mirror.ghproxy.com: `https://mirror.ghproxy.com`

**工作原理**：
```dockerfile
# 下载 install-php-extensions 脚本
RUN if [ -n "$GITHUB_PROXY" ]; then
      EXTENSIONS_URL="${GITHUB_PROXY}/https://github.com/mlocati/docker-php-extension-installer/releases/latest/download/install-php-extensions"
    else
      EXTENSIONS_URL="https://github.com/mlocati/docker-php-extension-installer/releases/latest/download/install-php-extensions"
    fi && \
    curl -sSLf -o /usr/local/bin/install-php-extensions $EXTENSIONS_URL
```

**生效时机**：
- 构建 Docker 镜像时从 GitHub 下载资源
- 特别适用于 PHP extension 安装脚本的下载

---

## 作用时机

### 镜像源在什么时候发挥作用？

| 阶段 | 使用的镜像源 | 说明 |
|------|------------|------|
| **Docker 拉取基础镜像** | Docker Registry | `docker pull php:8.5-fpm` |
| **Dockerfile 构建 - APT** | APT Mirror | `apt-get install` 安装系统依赖 |
| **Dockerfile 构建 - GitHub** | GitHub Proxy | 下载 `install-php-extensions` 等脚本 |
| **Dockerfile 构建 - Composer** | Composer Mirror | `composer install` 安装 PHP 依赖 |
| **Dockerfile 构建 - NPM** | NPM Registry | `npm install` 安装 Node 模块 |
| **宿主机 - Composer** | Composer Mirror | 用户在项目中执行 `composer install` |
| **宿主机 - NPM** | NPM Registry | 用户在项目中执行 `npm install`（通过 `.npmrc`） |

---

## 配置步骤

### 快速开始

1. **打开镜像源管理页面**
   - 点击左侧菜单 "🌐 镜像源"

2. **选择或自定义镜像源**
   - 每个类别都有预定义的加速镜像
   - 也可以选择 "✏️ 自定义..." 输入自己的镜像地址

3. **配置 Docker Registry**
   - 点击 "打开 Docker Desktop 设置" 按钮
   - 复制提供的 JSON 配置
   - 在 Docker Desktop → Settings → Docker Engine 中粘贴
   - 点击 "Apply & Restart"

4. **应用配置**
   - 点击页面右上角 "应用配置" 按钮
   - 配置会保存到 `.env` 文件

5. **重新构建镜像**
   ```bash
   # 停止当前容器
   docker compose down
   
   # 重新构建并启动
   docker compose up --build -d
   ```

---

## 常见问题

### Q1: 修改镜像源后为什么没有立即生效？

**A**: 镜像源配置在 **Docker 镜像构建时** 生效。如果你已经构建了镜像，需要重新构建：

```bash
docker compose up --build -d
```

或者先清理旧镜像：
```bash
docker compose down
docker system prune -f  # 清理未使用的镜像
docker compose up --build -d
```

---

### Q2: Docker Registry Mirror 配置后还是慢？

**A**: 可能的原因：
1. Docker Desktop 没有重启（配置后必须重启）
2. 镜像源本身不稳定，尝试更换其他镜像
3. 网络问题，测试连接是否正常

测试方法：
```bash
docker pull hello-world
```

---

### Q3: 如何验证镜像源是否生效？

**A**: 观察构建日志：

```bash
docker compose up --build
```

查看输出中是否有类似内容：
```
# APT 镜像
Get:1 https://mirrors.aliyun.com/debian trixie InRelease

# Composer 镜像
Using version ^1.0 for xxx from https://mirrors.aliyun.com/composer/

# NPM 镜像
npm http fetch GET 200 https://registry.npmmirror.com/xxx
```

---

### Q4: GitHub Proxy 会影响安全性吗？

**A**: GitHub Proxy 只是转发请求，不会修改内容。但建议：
1. 使用可信的代理服务（如 ghproxy.com）
2. 仅在需要时启用（下载大文件时）
3. 生产环境建议使用官方源

---

### Q5: 可以为不同项目配置不同的镜像源吗？

**A**: 可以！每个项目的 `.env` 文件独立，可以配置不同的镜像源：

```env
# 项目 A - 使用阿里云
APT_MIRROR=https://mirrors.aliyun.com/debian
COMPOSER_MIRROR=https://mirrors.aliyun.com/composer/

# 项目 B - 使用腾讯云
APT_MIRROR=https://mirrors.cloud.tencent.com/debian
COMPOSER_MIRROR=https://mirrors.cloud.tencent.com/composer/
```

---

### Q6: 如何恢复为官方默认源？

**A**: 在镜像源管理页面：
1. 选择 "🌐 官方默认（不加速）"
2. 点击 "应用配置"
3. 重新构建镜像

或者手动编辑 `.env` 文件，删除或清空相关配置项。

---

## 最佳实践

### 1. 国内开发环境推荐配置

```env
# .env 文件
APT_MIRROR=https://mirrors.aliyun.com/debian
COMPOSER_MIRROR=https://mirrors.aliyun.com/composer/
NPM_MIRROR=https://registry.npmmirror.com
GITHUB_PROXY=https://ghproxy.com
```

Docker Desktop:
```json
{
  "registry-mirrors": [
    "https://registry.cn-hangzhou.aliyuncs.com"
  ]
}
```

### 2. 团队协作

- 将 `.env` 加入 `.gitignore`（已默认配置）
- 提供 `.env.example` 作为模板
- 团队成员根据各自网络环境配置镜像源

### 3. CI/CD 环境

- 在 CI/CD 流水线中设置环境变量
- 使用稳定的镜像源（如官方源或企业内网镜像）
- 避免使用可能不稳定的公共代理

---

## 技术支持

如果遇到问题：
1. 检查网络连接
2. 测试镜像源可用性（点击"测试连接"按钮）
3. 查看 Docker 构建日志
4. 尝试更换其他镜像源

---

**最后更新**: 2026-04-17  
**版本**: PHP-Stack v2.0
