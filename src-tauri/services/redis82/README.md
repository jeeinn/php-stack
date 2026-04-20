# Redis 8.2 配置文件说明

## 📋 基本信息

- **版本**: Redis 8.2.5 (最新稳定版)
- **镜像**: `redis:8.2-alpine`
- **生成时间**: 2026-04-20
- **配置来源**: 
  - 官方默认配置: https://raw.githubusercontent.com/redis/redis/unstable/redis.conf
  - dnmp 优化配置: `dnmp/services/redis/redis-8.2.2.conf`

## 🔧 配置特点

### 1. 网络配置
```conf
bind 0.0.0.0              # 允许所有接口访问（Docker 容器内）
protected-mode no         # 禁用保护模式（Docker 网络隔离已提供安全性）
port 6379                 # 标准 Redis 端口
```

### 2. 持久化配置
- **RDB**: 启用快照持久化
  - 900秒内至少1个变化
  - 300秒内至少10个变化
  - 60秒内至少10000个变化
- **AOF**: 默认禁用（可根据需要启用）
  - 策略: everysec（每秒同步）

### 3. 性能优化
- TCP keepalive: 300秒
- TCP backlog: 511
- 最大客户端: 10000
- I/O 线程: 默认禁用（可根据 CPU 核心数启用）

### 4. Docker 适配
- `daemonize no`: 前台运行（适合 Docker）
- `logfile ""`: 输出到 stdout（便于日志收集）
- `dir ./`: 工作目录（通过 volume 映射）

## 📝 与旧版本的差异

### Redis 8.2 vs 7.2

| 配置项 | Redis 7.2 | Redis 8.2 | 说明 |
|--------|-----------|-----------|------|
| `proc-title-template` | ❌ 不支持 | ✅ 支持 | 新增进程标题模板 |
| `set-proc-title` | ❌ 不支持 | ✅ 支持 | 新增进程标题设置 |
| `aof-timestamp-enabled` | ❌ 不支持 | ✅ 支持 | 新增 AOF 时间戳 |
| `rdb-del-sync-files` | ❌ 不支持 | ✅ 支持 | 新增 RDB 加载策略 |
| `oom-score-adj` | ⚠️ 实验性 | ✅ 稳定 | OOM 调整更稳定 |
| `io-threads` | ✅ 支持 | ✅ 增强 | I/O 线程性能提升 |

### 重要变更

1. **进程管理改进**
   - 新增 `proc-title-template` 和 `set-proc-title`
   - 更好的进程监控和调试支持

2. **AOF 增强**
   - 新增 `aof-timestamp-enabled`
   - 支持 AOF 文件时间戳记录

3. **内存管理优化**
   - OOM Score 调整更加稳定
   - 改进的惰性删除机制

## 🚀 使用建议

### 开发环境
```conf
# 保持当前配置即可
appendonly no             # 禁用 AOF 提高性能
maxmemory-policy noeviction  # 不淘汰数据
```

### 生产环境
```conf
# 启用密码认证
requirepass your_strong_password_here

# 启用 AOF 持久化
appendonly yes
appendfsync everysec

# 设置内存限制
maxmemory 2gb
maxmemory-policy allkeys-lru

# 启用 I/O 线程（根据 CPU 核心数）
io-threads 4
io-threads-do-reads yes
```

### Docker Compose 配置示例
```yaml
services:
  redis82:
    image: redis:8.2-alpine
    container_name: ps-redis82
    ports:
      - "${REDIS82_PORT:-6379}:6379"
    volumes:
      - ./services/redis82/redis.conf:/usr/local/etc/redis/redis.conf
      - ${REDIS82_DATA_DIR:-./data/redis82}:/data
    command: redis-server /usr/local/etc/redis/redis.conf
    restart: unless-stopped
```

## 🔍 验证配置

```bash
# 测试配置文件语法
docker run --rm \
  -v $(pwd)/services/redis82/redis.conf:/etc/redis/redis.conf \
  redis:8.2-alpine \
  redis-server /etc/redis/redis.conf --test-memory 1

# 启动并验证
docker run --rm \
  -v $(pwd)/services/redis82/redis.conf:/etc/redis/redis.conf \
  redis:8.2-alpine \
  redis-server /etc/redis/redis.conf
```

## 📚 参考资料

- [Redis 8.2 官方文档](https://redis.io/docs/latest/)
- [Redis 配置详解](https://redis.io/docs/latest/operate/oss_and_stack/management/config/)
- [dnmp Redis 配置](../../../../../dnmp/services/redis/redis-8.2.2.conf)
- [Redis GitHub](https://github.com/redis/redis)

## 📅 更新历史

- **2026-04-20**: 初始版本，基于官方配置 + dnmp 优化
  - 从官方 GitHub 获取基础配置
  - 应用 dnmp 的性能优化参数
  - 适配 Docker 容器环境
  - 添加中文注释
