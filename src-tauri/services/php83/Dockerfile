# Dynamic base image support
# Users can override by setting PHP83_VERSION in .env (e.g., php:8.3-fpm-alpine)
ARG PHP_BASE_IMAGE=php:8.3-fpm
FROM ${PHP_BASE_IMAGE}

ARG TZ
ARG PHP_EXTENSIONS
ARG DEBIAN_MIRROR_DOMAIN
ARG COMPOSER_MIRROR
ARG GITHUB_PROXY

ENV TZ=${TZ}

# Set debian mirror
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
# Performance optimizations:
# - IPE_PROCESSOR_COUNT: Use all available CPU cores for parallel compilation
# - IPE_GD_WITHOUTAVIF: Skip AVIF support (requires slow Google source download)
# - IPE_ICU_EN_ONLY: Install English-only ICU library to reduce download size
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
ARG PUID=1000
ARG PGID=1000

# 把容器内 www-data 用户的 UID/GID 对齐宿主机，避免挂载目录里写出的文件属主不匹配。
#
# 这里修掉了原实现的两个坑：
#   1. 原条件是 `PUID != 1000 || PGID != 1000`，而默认值本身就是 1000 ——
#      等于默认配置下永远不执行，映射形同虚设。现在改为「与目标不一致才调整」。
#   2. 原实现只用 debian 的 groupmod/usermod。用户若把基础镜像换成 alpine 版
#      （如 php:8.2-fpm-alpine），构建会直接 `groupmod: not found` 失败。
#      现在按可用命令分发两种改法。
#
# 只在与目标不一致时才改动，保证默认构建层可缓存。
RUN set -eux; \
    if [ "$(id -u www-data)" != "${PUID}" ] || [ "$(id -g www-data)" != "${PGID}" ]; then \
      if command -v usermod >/dev/null 2>&1; then \
        groupmod -o -g "${PGID}" www-data; \
        usermod -o -u "${PUID}" -g "${PGID}" www-data; \
      elif command -v adduser >/dev/null 2>&1; then \
        deluser www-data; \
        # alpine 的 deluser 会连带删掉同名组，delgroup 再删会报 unknown group
        delgroup www-data 2>/dev/null || true; \
        addgroup -g "${PGID}" www-data; \
        adduser -u "${PUID}" -G www-data -s /bin/sh -D www-data; \
      else \
        echo "skip: 未识别的基础镜像，沿用镜像默认的 www-data 用户"; \
      fi; \
    fi; \
    echo "www-data uid=$(id -u www-data) gid=$(id -g www-data)"

WORKDIR /www
