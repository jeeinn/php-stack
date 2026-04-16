FROM php:7.4-fpm

# 安装并配置 Composer 镜像源
RUN curl -sS https://getcomposer.org/installer | php -- --install-dir=/usr/local/bin --filename=composer && \
    composer config -g repo.packagist composer https://mirrors.aliyun.com/composer

# 配置 PyPI 镜像源
RUN pip config set global.index-url https://mirrors.aliyun.com/pypi/simple

# 安装系统依赖
RUN apt-get update && apt-get install -y --no-install-recommends \
    libfreetype6-dev \
    libjpeg-dev \
    libmariadb-dev \
    libpng-dev && \
    rm -rf /var/lib/apt/lists/*

# 安装 PHP 核心扩展
RUN docker-php-ext-install -j$(nproc) mysqli
RUN docker-php-ext-install -j$(nproc) pdo_mysql
RUN docker-php-ext-configure gd --with-freetype --with-jpeg && \
    docker-php-ext-install -j$(nproc) gd
RUN docker-php-ext-install -j$(nproc) mbstring

# 安装 PECL 扩展
RUN pecl install redis-5.3.7 && docker-php-ext-enable redis

WORKDIR /var/www/html
