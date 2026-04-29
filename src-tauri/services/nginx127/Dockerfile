# Dynamic base image support
# Users can override by setting NGINX127_VERSION in .env (e.g., nginx:1.27, nginx:1.25-alpine)
ARG NGINX_BASE_IMAGE=nginx:1.27-alpine
FROM ${NGINX_BASE_IMAGE}

ARG PUID=1000
ARG PGID=1000

# Dynamically adjust nginx user ID to match host (Alpine Linux)
# Default nginx UID=100, GID=101 in Alpine
RUN if [ "${PUID}" != "1000" ] || [ "${PGID}" != "1000" ]; then \
      # Delete existing nginx group and user
      deluser nginx && \
      delgroup nginx && \
      # Create new group and user with specified IDs
      addgroup -g ${PGID} nginx && \
      adduser -u ${PUID} -G nginx -s /bin/sh -D nginx && \
      echo "Adjusted nginx to UID=${PUID}, GID=${PGID}"; \
    fi

WORKDIR /www

USER nginx
