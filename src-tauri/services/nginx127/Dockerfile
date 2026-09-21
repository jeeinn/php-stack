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

# 不能切换为 USER nginx：官方 nginx 镜像的 entrypoint 需要 root 权限
# （创建 /var/cache/nginx/*_temp 目录、改写 conf.d/default.conf），
# 且 80 端口只有 root 能 bind。以 nginx 身份启动会直接报
# [emerg] mkdir() "/var/cache/nginx/client_temp" failed (13: Permission denied)。
# worker 进程身份由 nginx.conf 的 `user nginx;` 决定，PUID/PGID 映射仍然生效：
# root 只跑 master，写文件（日志、缓存）的是 nginx(PUID:PGID)。
