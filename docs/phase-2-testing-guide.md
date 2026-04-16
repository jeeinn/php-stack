# Phase 2 功能测试验证指南

**版本**: v1.1  
**状态**: ✅ 已完成并待验证  
**创建日期**: 2026-04-16

---

## 📋 测试前准备

### 1. 环境要求
- ✅ Docker Desktop 已启动
- ✅ Node.js 和 npm 已安装
- ✅ Rust 工具链已安装

### 2. 清理旧数据（可选）
```bash
# 删除旧的 docker-compose.yml（如果存在）
rm docker-compose.yml

# 停止所有 ps- 开头的容器
docker stop $(docker ps -aq --filter "name=ps-")
docker rm $(docker ps -aq --filter "name=ps-")
```

---

## 🧪 测试方案

### 方案一：自动化单元测试（快速验证核心逻辑）

```bash
cd e:\study\php-stack\src-tauri
cargo test compose_manager
```

**预期输出：**
```
running 3 tests
test engine::compose_manager::tests::test_build_service_config_basic ... ok
test engine::compose_manager::tests::test_determine_dependencies ... ok
test engine::compose_manager::tests::test_extract_service_name ... ok

test result: ok. 3 passed; 0 failed
```

**验证内容：**
- ✅ 服务名提取正确（ps-php-8-2 → php）
- ✅ 依赖关系判断正确（PHP 依赖 MySQL/Redis）
- ✅ 服务配置生成正确（端口、网络、重启策略）

---

### 方案二：完整集成测试（推荐）

#### 步骤 1：启动开发服务器

```bash
npm run tauri dev
```

等待应用窗口打开。

---

#### 步骤 2：安装第一个服务（MySQL）

**操作：**
1. 在前端界面点击"软件中心"
2. 选择 **MySQL 8.0**
3. 点击"安装"
4. 等待安装完成（约 1-2 分钟，需要拉取镜像）

**验证点 A：检查容器是否运行**
```bash
docker ps --filter "name=ps-mysql"
```

应该看到 `ps-mysql-8-0` 容器处于 `Up` 状态。

**验证点 B：检查 docker-compose.yml 是否生成**
```bash
cat docker-compose.yml
```

**预期内容：**
```yaml
version: "3.8"
networks:
  php-stack-network:
    driver: bridge
    external: true
services:
  mysql:
    image: "mysql:8.0"
    container_name: "ps-mysql-8-0"
    networks:
      - "php-stack-network"
    ports:
      - "3306:3306"
    environment:
      MYSQL_ROOT_PASSWORD: "root"
      MYSQL_DATABASE: "default_db"
    restart: "unless-stopped"
```

**验证点 C：检查 Docker Compose 是否识别**
```bash
docker compose ps
```

应该显示 `ps-mysql-8-0` 服务。

---

#### 步骤 3：安装第二个服务（Redis）

**操作：**
1. 在前端界面选择 **Redis 7.0**
2. 点击"安装"
3. 等待安装完成

**验证点：**
```bash
cat docker-compose.yml
```

应该看到新增了 `redis` 服务配置。

---

#### 步骤 4：安装第三个服务（PHP）

**操作：**
1. 在前端界面选择 **PHP 8.2**
2. 点击"安装"
3. 等待安装完成

**验证点 A：检查依赖关系**
```bash
cat docker-compose.yml | grep -A 5 "php:"
```

应该看到：
```yaml
php:
  # ...
  depends_on:
    - "mysql"
    - "redis"
```

**验证点 B：测试服务间通信**
```bash
# 进入 PHP 容器
docker exec -it ps-php-8-2 bash

# 在容器内执行
ping mysql
ping redis
```

**预期结果：**
- ✅ `ping mysql` 成功（能解析到 MySQL 容器的 IP）
- ✅ `ping redis` 成功

**退出容器：**
```bash
exit
```

---

#### 步骤 5：卸载服务测试

**操作：**
1. 在前端界面找到 **Redis** 容器
2. 点击"卸载"
3. 确认卸载

**验证点：**
```bash
cat docker-compose.yml
```

**预期结果：**
- ❌ Redis 服务已从配置中移除
- ✅ MySQL 和 PHP 服务仍然存在

```bash
docker compose ps
```

应该只显示 MySQL 和 PHP 两个服务。

---

#### 步骤 6：手动触发 Compose 重建（调试命令）

如果你想在任何时候手动重建 docker-compose.yml，可以在浏览器控制台执行：

```javascript
// 打开浏览器开发者工具 (F12)
// 切换到 Console 标签
await window.__TAURI__.core.invoke('rebuild_compose_file')
```

**预期输出：**
```
"✅ docker-compose.yml 已重建: ./docker-compose.yml"
```

同时查看终端日志，应该看到：
```
[INFO] 🔧 手动触发 docker-compose.yml 重建，当前容器数: 2
[INFO] ✅ docker-compose.yml 已更新 (2 个服务)
```

---

### 方案三：命令行直接测试（高级用户）

如果你想绕过前端，直接通过 API 测试：

#### 1. 安装 MySQL
```powershell
# 使用 PowerShell 调用 Tauri API（需要先启动应用）
# 或者编写一个简单的测试脚本
```

#### 2. 检查生成的文件
```bash
ls -lh docker-compose.yml
cat docker-compose.yml
```

#### 3. 验证 Docker Compose 配置
```bash
# 验证 YAML 语法
docker compose config

# 查看服务列表
docker compose ps

# 查看网络
docker network inspect php-stack-network
```

---

## 🔍 常见问题排查

### 问题 1：docker-compose.yml 没有生成

**可能原因：**
- 容器安装失败
- 权限问题

**解决方法：**
```bash
# 检查容器是否真的创建了
docker ps -a --filter "name=ps-"

# 检查应用日志
# 在 npm run tauri dev 的终端窗口查看输出
```

---

### 问题 2：服务间无法通信

**可能原因：**
- 容器未加入统一网络

**解决方法：**
```bash
# 检查容器网络
docker inspect ps-php-8-2 | grep Networks -A 10

# 手动迁移容器到统一网络
# 在浏览器控制台执行
await window.__TAURI__.core.invoke('migrate_containers_to_network')
```

---

### 问题 3：端口冲突

**可能原因：**
- 宿主机端口已被占用

**解决方法：**
```bash
# 检查端口占用
netstat -ano | findstr :3306

# 修改端口映射（在前端重新安装时选择不同的端口）
```

---

### 问题 4：Docker Compose 不识别服务

**可能原因：**
- docker-compose.yml 格式错误
- 网络不存在

**解决方法：**
```bash
# 验证 YAML 格式
docker compose config

# 手动创建网络
docker network create php-stack-network

# 重新启动服务
docker compose up -d
```

---

## ✅ 验收标准

Phase 2 功能被认为**完全通过**，当满足以下条件：

- [ ] ✅ 单元测试全部通过（3/3）
- [ ] ✅ 安装单个服务后自动生成 docker-compose.yml
- [ ] ✅ 安装多个服务后正确生成依赖关系
- [ ] ✅ 卸载服务后自动更新配置文件
- [ ] ✅ 服务间可通过服务名互相访问（无需 IP）
- [ ] ✅ Docker Compose 能正确识别和管理所有服务
- [ ] ✅ 手动重建命令正常工作

---

## 📊 性能指标

| 操作 | 预期时间 | 说明 |
|------|---------|------|
| 安装首个服务 | 1-3 分钟 | 包含镜像拉取 |
| 安装后续服务 | 30-60 秒 | 镜像已缓存 |
| 生成 docker-compose.yml | < 1 秒 | 纯本地操作 |
| 应用 Compose 变更 | 2-5 秒 | Docker 内部操作 |
| 服务间 ping 测试 | < 10ms | 同一网络内 |

---

## 🎯 下一步

测试通过后，可以继续进行：

1. **Phase 3: 智能重启优化**
   - 实现更精细的依赖分析
   - 避免不必要的容器重启

2. **Phase 4: 前端增强**
   - 显示服务依赖关系图
   - 添加 docker-compose.yml 查看器
   - 网络拓扑可视化

---

## 📝 测试记录模板

复制此模板记录你的测试结果：

```markdown
### 测试记录 - 2026-04-16

**测试人员**: [你的名字]
**测试环境**: Windows 24H2, Docker Desktop 4.x

#### 测试结果

- [ ] 单元测试: ___/3 通过
- [ ] MySQL 安装: ✅ / ❌
- [ ] Redis 安装: ✅ / ❌
- [ ] PHP 安装: ✅ / ❌
- [ ] docker-compose.yml 生成: ✅ / ❌
- [ ] 服务间通信: ✅ / ❌
- [ ] 卸载功能: ✅ / ❌

#### 发现的问题

1. [问题描述]
   - 复现步骤: ...
   - 预期行为: ...
   - 实际行为: ...

#### 截图/日志

[粘贴相关截图或日志片段]
```

---

**最后更新**: 2026-04-16  
**维护者**: PHP-Stack Team
