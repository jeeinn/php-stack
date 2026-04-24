# Dynamic base image support
# Users can override by setting PHP74_VERSION in .env (e.g., php:7.4-fpm-alpine)
ARG PHP_BASE_IMAGE=php:7.4-fpm
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
WORKDIR /www
