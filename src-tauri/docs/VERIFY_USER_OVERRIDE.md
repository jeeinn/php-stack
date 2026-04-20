# 用户版本覆盖配置验证指南

## 📋 前置条件

确保您已经：
1. ✅ 在 UI 中编辑了版本覆盖配置
2. ✅ 看到"保存成功！重新应用配置后生效"的提示
3. ✅ `.user_version_overrides.json` 文件已更新

---

## 🔍 验证步骤

### 步骤 1: 确认配置文件内容

**文件位置**: 项目根目录 `.user_version_overrides.json`（与 `.env`、`docker-compose.yml` 同级）

**检查方法**:
```bash
# PowerShell
Get-Content .user_version_overrides.json

# 或使用文本编辑器打开
code .user_version_overrides.json
```

**预期内容示例**:
```json
{
  "redis": {
    "7.0": {
      "tag": "7.0-alpine-00",
      "description": "Redis 7.0 (稳定版)"
    }
  }
}
```

✅ **验证点**:
- 文件存在
- JSON 格式正确
- 包含您修改的版本和标签

---

### 步骤 2: 在 UI 中查看标记

1. 打开 **"软件设置"** 页面
2. 选择对应的服务类型（如 Redis）
3. 找到您修改的版本（如 7.0）

**预期显示**:
```
版本号: 7.0
Docker 镜像标签: 7.0-alpine-00 (自定义) ← 黄色标记
完整镜像名: redis:7.0-alpine-00
```

✅ **验证点**:
- 标签显示为您设置的值
- 有 **(自定义)** 黄色标记
- 完整镜像名正确

---

### 步骤 3: 应用配置

**重要**: 修改后必须重新应用配置才能生效！

1. 切换到 **"环境配置"** 页面
2. 点击 **"应用配置"** 按钮
3. 等待配置生成完成（通常 2-5 秒）
4. 看到"配置应用成功"提示

**后台执行的操作**:
```
1. 读取项目根目录的 .user_version_overrides.json
2. 合并 version_manifest.json 的默认配置
3. 生成 .env 文件（使用自定义标签）
4. 生成 docker-compose.yml
5. 复制服务配置文件到 services/ 目录
```

---

### 步骤 4: 检查生成的 .env 文件

**文件位置**: 项目根目录 `.env`

**检查方法**:
```bash
# PowerShell - 查找特定服务的配置
Get-Content .env | Select-String "REDIS70"

# 或查看完整文件
code .env
```

**预期输出**:
```env
# Redis 7.0 配置
REDIS70_VERSION=7.0-alpine-00    ← 这里应该是自定义标签
REDIS70_HOST_PORT=6379
REDIS70_CONF_FILE=./services/redis70/redis.conf
REDIS70_DATA_DIR=./data/redis70
```

✅ **关键验证点**:
- `REDIS70_VERSION` 的值是 `7.0-alpine-00`（您的自定义标签）
- 如果不是，说明配置未正确应用

---

### 步骤 5: 检查 docker-compose.yml

**文件位置**: 项目根目录 `docker-compose.yml`

**检查方法**:
```bash
# 查看 Redis 70 服务配置
Get-Content docker-compose.yml | Select-String -Pattern "redis70:" -Context 0,5
```

**预期输出**:
```yaml
  redis70:
    image: redis:${REDIS70_VERSION}    ← 使用环境变量
    container_name: ps-redis70
    ports:
      - "${REDIS70_HOST_PORT}:6379"
```

✅ **验证点**:
- 使用 `${REDIS70_VERSION}` 变量
- 该变量在 .env 中定义为自定义标签

---

### 步骤 6: 启动容器并验证

#### 6.1 启动服务

```bash
# 启动所有服务
docker compose up -d

# 或只启动 Redis 70
docker compose up -d redis70
```

#### 6.2 检查容器状态

```bash
# 查看所有容器
docker ps | findstr redis

# 预期输出:
# CONTAINER ID   IMAGE                  STATUS
# xxxxxxxxxxxx   redis:7.0-alpine-00    Up 10 seconds
```

✅ **关键验证点**:
- **IMAGE 列**显示为 `redis:7.0-alpine-00`
- 状态为 `Up`（运行中）

#### 6.3 验证镜像是否正确拉取

```bash
# 查看本地镜像
docker images | findstr redis

# 预期输出:
# REPOSITORY   TAG              IMAGE ID
# redis        7.0-alpine-00    xxxxxxxxxxxx
```

⚠️ **如果镜像不存在**:
```bash
# 手动拉取测试
docker pull redis:7.0-alpine-00

# 如果拉取失败，说明标签不存在
# Error response from daemon: manifest for redis:7.0-alpine-00 not found
```

---

### 步骤 7: 功能测试

#### 7.1 连接 Redis 测试

```bash
# 进入容器
docker exec -it ps-redis70 redis-cli

# 测试基本命令
127.0.0.1:6379> PING
PONG

127.0.0.1:6379> SET test_key "Hello World"
OK

127.0.0.1:6379> GET test_key
"Hello World"

127.0.0.1:6379> INFO server | findstr redis_version
redis_version:7.0.x

127.0.0.1:6379> exit
```

✅ **验证点**:
- 能够成功连接
- Redis 版本是 7.0.x
- 基本命令正常工作

---

## 🐛 常见问题排查

### 问题 1: .env 文件中仍然是默认标签

**症状**:
```env
REDIS70_VERSION=7.0-alpine  # 应该是 7.0-alpine-00
```

**原因**: 配置未正确应用

**解决**:
1. 检查 `.user_version_overrides.json` 格式是否正确
2. 重新点击"应用配置"按钮
3. 查看控制台是否有错误信息
4. 重启应用后再次尝试

---

### 问题 2: 容器启动失败

**症状**:
```bash
docker compose up -d redis70
# Error: manifest for redis:7.0-alpine-00 not found
```

**原因**: 指定的镜像标签不存在

**解决**:
```bash
# 1. 验证标签是否存在
docker pull redis:7.0-alpine-00

# 2. 如果不存在，修正标签
# 在 UI 中编辑为正确的标签，如 7.0-alpine

# 3. 重新应用配置
# 4. 重新启动容器
docker compose down redis70
docker compose up -d redis70
```

---

### 问题 3: UI 中没有显示"(自定义)"标记

**症状**: 版本列表中看不到黄色标记

**原因**: 
- 前端缓存未刷新
- 配置文件格式错误

**解决**:
1. 刷新页面（F5）
2. 检查浏览器控制台是否有错误
3. 验证 JSON 文件格式：
   ```bash
   # 使用 jq 验证
   cat src-tauri/.user_version_overrides.json | jq .
   ```

---

### 问题 4: 修改后重启应用失效

**症状**: 重启应用后配置丢失

**原因**: `.user_version_overrides.json` 被删除或覆盖

**解决**:
1. 检查文件是否存在：
   ```bash
   Test-Path src-tauri/.user_version_overrides.json
   ```

2. 如果不存在，重新配置
3. 建议备份配置文件：
   ```bash
   Copy-Item src-tauri/.user_version_overrides.json `
             src-tauri/.user_version_overrides.json.backup
   ```

---

## 📊 验证清单

使用此清单确保配置完全生效：

- [ ] **配置文件存在**: `src-tauri/.user_version_overrides.json`
- [ ] **JSON 格式正确**: 无语法错误
- [ ] **UI 显示标记**: 版本旁边有"(自定义)"黄色标记
- [ ] **配置已应用**: 点击过"应用配置"按钮
- [ ] **.env 文件更新**: `REDIS70_VERSION=7.0-alpine-00`
- [ ] **docker-compose 正确**: 使用 `${REDIS70_VERSION}` 变量
- [ ] **容器已启动**: `docker ps` 显示运行中
- [ ] **镜像正确**: `docker images` 显示自定义标签
- [ ] **功能正常**: `redis-cli` 能够连接并执行命令

---

## 💡 快速验证脚本

创建一个 PowerShell 脚本快速验证：

```powershell
# verify-override.ps1

Write-Host "=== 验证用户版本覆盖配置 ===" -ForegroundColor Cyan

# 1. 检查配置文件（项目根目录，与 .env 同级）
if (Test-Path ".user_version_overrides.json") {
    Write-Host "✅ 配置文件存在" -ForegroundColor Green
    Get-Content ".user_version_overrides.json" | ConvertFrom-Json | ConvertTo-Json
} else {
    Write-Host "❌ 配置文件不存在" -ForegroundColor Red
    exit 1
}

# 2. 检查 .env 文件
if (Test-Path ".env") {
    Write-Host "`n✅ .env 文件存在" -ForegroundColor Green
    $redisConfig = Get-Content ".env" | Select-String "REDIS70_VERSION"
    if ($redisConfig) {
        Write-Host "   $redisConfig" -ForegroundColor Yellow
    }
} else {
    Write-Host "`n❌ .env 文件不存在，请先应用配置" -ForegroundColor Red
}

# 3. 检查容器状态
$container = docker ps --filter "name=ps-redis70" --format "{{.Image}}"
if ($container) {
    Write-Host "`n✅ 容器运行中" -ForegroundColor Green
    Write-Host "   镜像: $container" -ForegroundColor Yellow
} else {
    Write-Host "`n⚠️  容器未运行" -ForegroundColor Yellow
    Write-Host "   运行: docker compose up -d redis70" -ForegroundColor Gray
}

Write-Host "`n=== 验证完成 ===" -ForegroundColor Cyan
```

**使用方法**:
```powershell
.\verify-override.ps1
```

---

## 📚 相关文档

- [USER_OVERRIDE_GUIDE.md](./USER_OVERRIDE_GUIDE.md) - 用户版本覆盖功能使用指南
- [VERSION_MANIFEST.md](./VERSION_MANIFEST.md) - 版本清单系统详解
- [ARCHITECTURE.md](../ARCHITECTURE.md) - 系统架构文档

---

**最后更新**: 2026-04-20  
**适用版本**: PHP-Stack V2.1+
