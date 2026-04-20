ARG PHP_VERSION
FROM php:7.4-fpm-alpine

ARG TZ
ARG PHP_EXTENSIONS
ARG DEBIAN_MIRROR_DOMAIN
ARG COMPOSER_MIRROR
ARG NPM_REGISTRY
ARG GITHUB_PROXY

ENV TZ=${TZ}

# Set alpine mirror (if configured)
# Note: Alpine uses apk, not apt-get

# Install required packages
RUN apk add --no-cache tzdata curl

# Install PHP extensions (with GitHub proxy if configured)
RUN if [ -n "$GITHUB_PROXY" ]; then \
      EXTENSIONS_URL="${GITHUB_PROXY}/https://github.com/mlocati/docker-php-extension-installer/releases/latest/download/install-php-extensions"; \
    else \
      EXTENSIONS_URL="https://github.com/mlocati/docker-php-extension-installer/releases/latest/download/install-php-extensions"; \
    fi && \
    curl -sSLf -o /usr/local/bin/install-php-extensions $EXTENSIONS_URL && \
    chmod +x /usr/local/bin/install-php-extensions && \
    install-php-extensions $(echo $PHP_EXTENSIONS | tr ',' ' ')

# Configure Composer mirror
RUN if [ -n "$COMPOSER_MIRROR" ] && [ "$COMPOSER_MIRROR" != "https://packagist.org" ]; then \
      composer config -g repo.packagist composer $COMPOSER_MIRROR; \
    fi

# Configure NPM registry
RUN if [ -n "$NPM_REGISTRY" ] && [ "$NPM_REGISTRY" != "https://registry.npmjs.org" ]; then \
      npm config set registry $NPM_REGISTRY; \
    fi

# Install composer and change it's cache home
RUN if [ -n "$GITHUB_PROXY" ]; then \
      COMPOSER_URL="${GITHUB_PROXY}/https://getcomposer.org/installer"; \
    else \
      COMPOSER_URL="https://getcomposer.org/installer"; \
    fi && \
    curl -sS $COMPOSER_URL | php -- --install-dir=/usr/local/bin --filename=composer
ENV COMPOSER_HOME=/tmp/composer

# Set work directory
WORKDIR /www
