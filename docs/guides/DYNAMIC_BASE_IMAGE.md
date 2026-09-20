# 动态基础镜像切换功能说明

## 概述

PHP-Stack v0.1.0 引入了**动态基础镜像切换**功能，允许用户通过修改 `.env` 文件或 `.user_version_overrides.json` 来自定义 PHP 容器的基础镜像标签。

## 工作原理

### 1. 数据流向

```
用户配置 → .env / .user_version_overrides.json
    ↓
config_generator.rs 读取完整镜像标签（如 php:8.2-fpm-alpine）
    ↓
写入 .env: PHP82_VERSION=php:8.2-fpm-alpine
    ↓
生成 docker-compose.yml:
  build:
    args:
      PHP_BASE_IMAGE: "${PHP82_VERSION}"
    ↓
Docker BuildKit 传递参数到 Dockerfile
    ↓
Dockerfile:
  ARG PHP_BASE_IMAGE=php:8.2-fpm
  FROM ${PHP_BASE_IMAGE}
    ↓
最终构建的容器基于用户指定的镜像
```

### 2. 配置文件示例

#### `.env` 文件
```env
# PHP 8.2 使用 Alpine 版本（更小体积）
PHP82_VERSION=php:8.2-fpm-alpine

# PHP 7.4 使用默认 Debian 版本
PHP74_VERSION=php:7.4-fpm

# 其他配置...
PHP82_HOST_PORT=9082
PHP82_EXTENSIONS=pdo_mysql,gd,curl
```

#### `docker-compose.yml` 片段
```yaml
services:
  php82:
    build:
      context: ./services/php82
      args:
        PHP_BASE_IMAGE: "${PHP82_VERSION}"  # 从 .env 读取
        PHP_EXTENSIONS: "${PHP82_EXTENSIONS}"
        TZ: "${TZ}"
        DEBIAN_MIRROR_DOMAIN: "${APT_MIRROR:-deb.debian.org}"
        COMPOSER_MIRROR: "${COMPOSER_MIRROR:-https://packagist.org}"
        GITHUB_PROXY: "${GITHUB_PROXY:-}"
    container_name: ps-php82
    expose:
      - 9000
    volumes:
      - ${SOURCE_DIR}:/www/:rw
      - ${PHP82_PHP_CONF_FILE}:/usr/local/etc/php/php.ini
      - ${PHP82_FPM_CONF_FILE}:/usr/local/etc/php-fpm.d/www.conf
      - ${PHP82_LOG_DIR}:/var/log/php
    restart: always
    networks:
      - php-stack-network
```

#### `Dockerfile` (php82)
```dockerfile
# Dynamic base image support
# Users can override by setting PHP82_VERSION in .env (e.g., php:8.2-fpm-alpine)
ARG PHP_BASE_IMAGE=php:8.2-fpm
FROM ${PHP_BASE_IMAGE}

ARG TZ
ARG PHP_EXTENSIONS
ARG DEBIAN_MIRROR_DOMAIN
ARG COMPOSER_MIRROR
ARG GITHUB_PROXY

ENV TZ=${TZ}

# Set debian mirror (for domestic acceleration)
RUN DEBIAN_HOST=$(echo "${DEBIAN_MIRROR_DOMAIN:-deb.debian.org}" | sed 's|https\?://||' | cut -d'/' -f1) && \
    if [ -f /etc/apt/sources.list.d/debian.sources ]; then \
      sed -i "s|deb.debian.org|${DEBIAN_HOST}|g" /etc/apt/sources.list.d/debian.sources; \
    elif [ -f /etc/apt/sources.list ]; then \
      sed -i "s|deb.debian.org|${DEBIAN_HOST}|g" /etc/apt/sources.list; \
    fi

# Install required packages
RUN export option="--no-install-recommends -y" \
    && apt-get $option update \
    && apt-get $option install tzdata curl

# Clean up the package list to reduce image size
RUN apt-get clean && rm -rf /var/lib/apt/lists/*

# Install PHP extensions (with GitHub proxy if configured)
RUN if [ -n "$GITHUB_PROXY" ]; then \
      EXTENSIONS_URL="${GITHUB_PROXY}/https://github.com/mlocati/docker-php-extension-installer/releases/latest/download/install-php-extensions"; \
    else \
      EXTENSIONS_URL="https://github.com/mlocati/docker-php-extension-installer/releases/latest/download/install-php-extensions"; \
    fi && \
    curl --connect-timeout 10 -sSLf -o /usr/local/bin/install-php-extensions $EXTENSIONS_URL && \
    chmod +x /usr/local/bin/install-php-extensions && \
    IPE_PROCESSOR_COUNT=$(nproc) \
    IPE_GD_WITHOUTAVIF=1 \
    IPE_ICU_EN_ONLY=1 \
    install-php-extensions $(echo $PHP_EXTENSIONS | tr ',' ' ')

# Install composer and change it's cache home
RUN curl --connect-timeout 10 -sS https://getcomposer.org/installer | php -- --install-dir=/usr/local/bin --filename=composer
ENV COMPOSER_HOME=/tmp/composer

# Configure Composer mirror
RUN if [ -n "$COMPOSER_MIRROR" ] && [ "$COMPOSER_MIRROR" != "https://packagist.org" ]; then \
      composer config -g repo.packagist composer $COMPOSER_MIRROR; \
    fi

# Set work directory
WORKDIR /www
```

## 使用方法

### 方法 1：通过前端界面修改（推荐）

1. 打开 **环境配置** 页面
2. 选择 PHP 版本（如 8.2）
3. 点击 **高级设置** 或 **自定义镜像标签**
4. 输入完整的镜像标签（如 `php:8.2-fpm-alpine`）
5. 点击 **应用配置**

系统会自动：
- 更新 `.user_version_overrides.json`
- 重新生成 `.env` 和 `docker-compose.yml`
- 下次启动时使用新的基础镜像

### 方法 2：手动编辑 `.env` 文件

直接编辑项目根目录的 `.env` 文件：

```env
# 将 PHP 8.2 切换到 Alpine 版本
PHP82_VERSION=php:8.2-fpm-alpine

# 将 PHP 7.4 切换到特定小版本
PHP74_VERSION=php:7.4.33-fpm
```

然后重新运行：
```bash
docker compose down
docker compose up -d --build
```

### 方法 3：使用 `.user_version_overrides.json`

创建或编辑 `.user_version_overrides.json`：

```json
{
  "php": {
    "8.2": {
      "image": "php",
      "tag": "8.2-fpm-alpine"
    },
    "7.4": {
      "image": "php",
      "tag": "7.4.33-fpm"
    }
  }
}
```

## 常见用例

### 1. 减小镜像体积（使用 Alpine）

Alpine 版本比 Debian 版本小约 50-70 MB：

```env
PHP82_VERSION=php:8.2-fpm-alpine
PHP81_VERSION=php:8.1-fpm-alpine
```

**注意**：Alpine 使用 `apk` 包管理器，某些扩展可能需要额外配置。

### 2. 锁定特定小版本

```env
# 锁定到 PHP 8.2.15
PHP82_VERSION=php:8.2.15-fpm

# 锁定到 PHP 7.4.33（最后一个 7.4 版本）
PHP74_VERSION=php:7.4.33-fpm
```

### 3. 使用自定义构建的镜像

```env
# 使用私有仓库的镜像
PHP82_VERSION=registry.example.com/php:8.2-custom

# 使用本地构建的镜像
PHP82_VERSION=my-php:8.2-dev
```

### 4. 测试新版 PHP

```env
# 测试 PHP 8.3 RC 版本
PHP83_VERSION=php:8.3.0RC3-fpm

# 测试 PHP 8.4 开发版
PHP84_VERSION=php:8.4-dev-fpm
```

## 技术细节

### ARG 变量作用域

根据 Docker 官方文档：
- **在 FROM 之前定义的 ARG** 只能用于 FROM 指令
- **如果要在 FROM 之后使用**，需要重新声明（不赋值）

我们的 Dockerfile 遵循这个规则：

```dockerfile
# 第一个 ARG：仅用于 FROM
ARG PHP_BASE_IMAGE=php:8.2-fpm
FROM ${PHP_BASE_IMAGE}

# 第二个 ARG：重新声明以在后续指令中使用（如果需要）
# ARG PHP_BASE_IMAGE  # 当前不需要在 RUN 中使用，所以省略
```

### docker-compose build args 传递

Docker Compose 会将 `build.args` 中的变量传递给 Dockerfile 的 ARG：

```yaml
build:
  args:
    PHP_BASE_IMAGE: "${PHP82_VERSION}"  # 从 .env 读取
```

等价于：
```bash
docker build --build-arg PHP_BASE_IMAGE=php:8.2-fpm-alpine ...
```

### 默认值机制

如果用户没有指定 `PHP_BASE_IMAGE`，Dockerfile 会使用默认值：

```dockerfile
ARG PHP_BASE_IMAGE=php:8.2-fpm  # 默认值
FROM ${PHP_BASE_IMAGE}
```

这确保了即使 `.env` 中没有定义该变量，构建也能正常进行。

## 验证方法

### 1. 检查生成的 `.env` 文件

```bash
cat .env | grep PHP82_VERSION
# 应该输出: PHP82_VERSION=php:8.2-fpm-alpine
```

### 2. 检查生成的 `docker-compose.yml`

```bash
cat docker-compose.yml | grep -A 5 "php82:"
# 应该看到:
#   php82:
#     build:
#       args:
#         PHP_BASE_IMAGE: "${PHP82_VERSION}"
```

### 3. 查看实际使用的镜像

```bash
docker inspect ps-php82 | grep Image
# 应该显示你指定的镜像标签
```

### 4. 验证基础镜像类型

```bash
# 进入容器检查操作系统
docker exec -it ps-php82 cat /etc/os-release

# Debian 版本会显示:
# PRETTY_NAME="Debian GNU/Linux 12 (bookworm)"

# Alpine 版本会显示:
# NAME="Alpine Linux"
```

## 注意事项

### 1. Alpine vs Debian 兼容性

| 特性 | Debian | Alpine |
|------|--------|--------|
| 包管理器 | apt-get | apk |
| 镜像大小 | ~150 MB | ~80 MB |
| glibc | ✅ 原生支持 | ❌ 使用 musl libc |
| 扩展兼容性 | ✅ 最佳 | ⚠️ 部分扩展需编译 |
| 调试工具 | ✅ 丰富 | ❌ 最小化 |

**建议**：
- 生产环境：优先使用 Debian（更好的兼容性）
- CI/CD 或资源受限环境：可考虑 Alpine（更小的体积）

### 2. 扩展安装差异

某些 PHP 扩展在 Alpine 上需要不同的依赖：

```dockerfile
# Debian (默认)
RUN apt-get install -y libpng-dev libjpeg-dev

# Alpine
RUN apk add --no-cache libpng-dev libjpeg-turbo-dev
```

当前我们的 Dockerfile 主要针对 Debian 优化。如果使用 Alpine，可能需要调整扩展安装逻辑。

### 3. 缓存失效

修改基础镜像会导致 Docker 层缓存失效，下次构建会重新下载基础镜像和安装所有扩展。

**建议**：
- 确定最终镜像标签后再频繁构建
- 使用 `docker compose build --parallel` 加速多服务构建

## 故障排查

### 问题 1：构建失败，提示找不到镜像

**症状**：
```
ERROR: failed to solve: php:8.2-fpm-alpine: not found
```

**原因**：镜像标签不存在或拼写错误

**解决**：
1. 访问 [Docker Hub](https://hub.docker.com/_/php/tags) 确认标签存在
2. 检查 `.env` 中的拼写是否正确
3. 尝试拉取镜像验证：
   ```bash
   docker pull php:8.2-fpm-alpine
   ```

### 问题 2：扩展安装失败（Alpine）

**症状**：
```
ERROR: unable to select packages:
  php8-gd (no such package):
```

**原因**：Alpine 使用不同的包命名规则

**解决**：
1. 暂时切换回 Debian 版本
2. 或者修改 Dockerfile 适配 Alpine：
   ```dockerfile
   RUN apk add --no-cache $PHPIZE_DEPS libpng-dev libjpeg-turbo-dev
   ```

### 问题 3：`.env` 修改后未生效

**原因**：Docker Compose 缓存了旧的环境变量

**解决**：
```bash
# 停止并删除容器
docker compose down

# 重新构建（不使用缓存）
docker compose build --no-cache

# 重新启动
docker compose up -d
```

## 未来改进方向

1. **前端 UI 支持**：在环境配置页面添加"基础镜像类型"下拉框（Debian / Alpine）
2. **自动适配逻辑**：检测 Alpine 镜像时自动调整扩展安装命令
3. **镜像预检查**：在应用配置前验证镜像是否存在
4. **性能对比报告**：显示不同基础镜像的大小和构建时间差异

## 参考资料

- [Dockerfile ARG 指令官方文档](https://docs.docker.com/reference/dockerfile/#arg)
- [Docker Compose build args](https://docs.docker.com/compose/compose-file/build/#args)
- [PHP 官方 Docker 镜像](https://hub.docker.com/_/php)
- [Alpine vs Debian 对比](https://nickjanetakis.com/blog/alpine-linux-vs-debian-for-docker-images)
