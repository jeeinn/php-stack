# Dynamic base image support
# Users can override by setting NGINX125_VERSION in .env (e.g., nginx:1.25, nginx:1.24-alpine)
ARG NGINX_BASE_IMAGE=nginx:1.25-alpine
FROM ${NGINX_BASE_IMAGE}

ARG PUID=1000
ARG PGID=1000

# 把容器内 nginx 用户的 UID/GID 对齐宿主机，避免挂载目录里写出的文件属主不匹配。
#
# 这里修掉了原实现的两个坑：
#   1. 原条件是 `PUID != 1000 || PGID != 1000`，而默认值本身就是 1000 ——
#      等于默认配置下永远不执行，映射形同虚设。现在改为「与目标不一致才调整」。
#   2. 原实现只用了 alpine 的 deluser/adduser。用户若在 .env 里把基础镜像换成
#      debian 版（如 nginx:1.28），构建会直接 `deluser: not found` 失败。
#      现在按基础镜像分发：alpine 用 deluser/adduser，debian 用 groupmod/usermod。
#
# 只在与目标不一致时才改动，保证默认构建层可缓存、不产生无谓的镜像层差异。
RUN set -eux; \
    if [ -f /etc/alpine-release ]; then \
      if [ "$(id -u nginx)" != "${PUID}" ] || [ "$(id -g nginx)" != "${PGID}" ]; then \
        deluser nginx; \
        # alpine 的 deluser 会连带删掉同名组，delgroup 再删会报 unknown group
        delgroup nginx 2>/dev/null || true; \
        addgroup -g "${PGID}" nginx; \
        adduser -u "${PUID}" -G nginx -s /bin/sh -D nginx; \
      fi; \
    elif command -v usermod >/dev/null 2>&1; then \
      if [ "$(id -g nginx)" != "${PGID}" ]; then groupmod -o -g "${PGID}" nginx; fi; \
      if [ "$(id -u nginx)" != "${PUID}" ]; then usermod -o -u "${PUID}" -g "${PGID}" nginx; fi; \
    else \
      echo "skip: 未识别的基础镜像，沿用镜像默认的 nginx 用户"; \
    fi; \
    echo "nginx uid=$(id -u nginx) gid=$(id -g nginx)"

WORKDIR /www

# 不能切换为 USER nginx：官方 nginx 镜像的 entrypoint 需要 root 权限
# （创建 /var/cache/nginx/*_temp 目录、改写 conf.d/default.conf），
# 且 80 端口只有 root 能 bind。以 nginx 身份启动会直接报
# [emerg] mkdir() "/var/cache/nginx/client_temp" failed (13: Permission denied)。
# worker 进程身份由 nginx.conf 的 `user nginx;` 决定，PUID/PGID 映射仍然生效：
# root 只跑 master，写文件（日志、缓存）的是 nginx(PUID:PGID)。
