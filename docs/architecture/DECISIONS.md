# PHP-Stack 关键技术决策

> **版本**: v0.2.0 (2026-04-27)  
> ↩ [返回主架构文档](./ARCHITECTURE.md)

本文档记录项目中的重要架构决策（ADR 风格），每条决策包含问题背景、方案对比和选择理由。

---

## 📋 目录

- [ADR-1: 版本清单系统设计](#adr-1-版本清单系统设计)
- [ADR-2: 用户覆盖机制](#adr-2-用户覆盖机制)
- [ADR-3: docker-compose 变量插值](#adr-3-docker-compose-变量插值)
- [ADR-4: Docker API 端口检测](#adr-4-docker-api-端口检测)
- [ADR-5: 循环检测等待容器停止](#adr-5-循环检测等待容器停止)
- [ADR-6: ConfirmDialog 禁止外部关闭](#adr-6-confirmdialog-禁止外部关闭)
- [ADR-7: 配置备份采用 ZIP 打包](#adr-7-配置备份采用-zip-打包)
- [ADR-8: 动态基础镜像切换](#adr-8-动态基础镜像切换)
- [ADR-9: version_manifest.json 扁平化重构](#adr-9-version_manifestjson-扁平化重构)

---

## ADR-1: 版本清单系统设计

**问题**: 用户选择版本号 `8.4`，但 Docker Hub 标签格式不一致（PHP: `8.4-fpm`, Redis: `7.2-alpine`），新版本发布时需多处修改代码。

**方案**: 集中管理版本映射在 `version_manifest.json`，配置生成器自动查询正确的标签。

**优势**:
- ✅ 解耦用户界面与 Docker 标签
- ✅ 单一数据源，易于维护
- ✅ 支持 EOL 检测和版本推荐
- ✅ 添加新版本只需编辑 JSON 文件（无需修改 Rust 代码）

---

## ADR-2: 用户覆盖机制

**设计理念**: 默认配置由开发者维护（安全性），高级用户可自定义（灵活性）。

**实现策略**:
- `.user_version_overrides.json` 中按 manifest ID 覆盖 `image_tag`
- `get_merged_entry()` 合并逻辑：用户覆盖仅替换 `image_tag`（和可选 `description`），其他字段保持 manifest 默认值
- 支持保存/删除/重置操作

**覆盖文件格式（v0.2.0）**:
```json
{
  "php": {
    "php82": {
      "image_tag": "php:8.2-fpm-alpine",
      "description": "使用 Alpine 基础镜像"
    }
  }
}
```

**优先级顺序**:
1. `.user_version_overrides.json` 用户覆盖配置（最高）
2. `version_manifest.json` 默认清单
3. Dockerfile 中的硬编码默认值（兜底）

---

## ADR-3: docker-compose 变量插值

**方案**: 使用 `${VAR}` 插值语法。

```yaml
mysql84:
  image: mysql:${MYSQL84_VERSION}
  ports:
    - "${MYSQL84_HOST_PORT}:3306"
```

**优势**:
- ✅ `.env` 和 `docker-compose.yml` 解耦
- ✅ 修改配置无需重新生成 compose 文件
- ✅ 符合 Docker Compose 最佳实践

---

## ADR-4: Docker API 端口检测

**问题**: 传统方法使用 `netstat`/`lsof` 检查宿主机端口，跨平台兼容性差，需要管理员权限，显示进程名而非容器名。

**方案**: 使用 Docker API (`list_all_running_containers`)。

**优势**:
- ✅ 完全跨平台（Docker API 统一接口）
- ✅ 精准定位（显示容器名、镜像、ID）
- ✅ 无需系统特定权限
- ✅ 简化实现（减少 90+ 行后端代码）

---

## ADR-5: 循环检测等待容器停止

**问题**: 硬编码等待时间（如 `sleep(2)`）不可靠，可能不足或过长。

**方案**: 循环检测 ps- 容器状态，最多 10 次，每次间隔 1 秒。

```rust
for attempt in 1..=10 {
    let ps_containers = manager.list_ps_containers().await?;
    let running: Vec<_> = ps_containers.iter()
        .filter(|c| c.state.contains("running"))
        .collect();
    if running.is_empty() { break; }
    tokio::time::sleep(Duration::from_secs(1)).await;
}
```

**优势**:
- ✅ 自适应：不同机器、不同容器数量都能准确判断
- ✅ 高效：容器停止后立即继续
- ✅ 可靠：基于 Docker API 实际状态
- ✅ 友好：超时时显示未停止的容器列表

---

## ADR-6: ConfirmDialog 禁止外部关闭

**问题**: 点击遮罩层触发 `handleCancel`，关键确认操作容易误触丢失。

**方案**: 移除 `@click.self` 事件处理器，强制用户通过按钮选择。

**影响范围**: 端口冲突确认、配置覆盖确认、删除/停止等危险操作。

**优势**:
- ✅ 防止误触
- ✅ 强制确认，减少操作失误
- ✅ 支持键盘操作（ESC 取消，Enter 确认）

---

## ADR-7: 配置备份采用 ZIP 打包

**问题**: 旧方案使用文件重命名（`.env` → `.env_20260424_180000`），产生多个分散文件，需要复杂回滚逻辑。

**方案**: 使用 `zip::ZipWriter` 打包成单一 ZIP 文件（Deflated 压缩）。

| 维度 | 旧方案（重命名） | 新方案（ZIP 打包） |
|------|----------------|------------------|
| **文件管理** | 多个分散文件 | 单一 ZIP 文件 |
| **磁盘占用** | 无压缩 | Deflated 压缩，节省 60-80% |
| **错误处理** | 复杂回滚逻辑 | 失败即删除 ZIP |
| **容错性** | 一处失败全盘皆输 | 单文件失败不影响其他 |
| **清理难度** | 需删除多个文件 | 只需删除一个 ZIP |

**设计原则**:
1. 简化优于复杂：移除复杂回滚逻辑
2. 容错优于严格：允许部分文件失败
3. 用户友好优先：整洁的工作目录

---

## ADR-8: 动态基础镜像切换

**问题**: 用户需求多样化（Debian/Alpine、版本锁定、私有仓库），硬编码 `FROM` 指令灵活性不足。

**方案**: Docker `ARG` + `FROM` 变量机制。

**数据流**:
```
用户配置 (.user_version_overrides.json)
    → config_generator.rs 获取 entry.image_tag
    → 写入 .env: PHP82_VERSION=php:8.2-fpm-alpine
    → docker-compose.yml: PHP_BASE_IMAGE="${PHP82_VERSION}"
    → Dockerfile: ARG PHP_BASE_IMAGE; FROM ${PHP_BASE_IMAGE}
    → 最终容器基于用户指定的镜像
```

**使用场景**:

| 场景 | .env 配置 |
|------|----------|
| 切换到 Alpine | `PHP82_VERSION=php:8.2-fpm-alpine` |
| 锁定小版本 | `PHP82_VERSION=php:8.2.15-fpm` |
| 私有仓库 | `PHP82_VERSION=registry.example.com/php:8.2-custom` |
| 测试新版 | `PHP83_VERSION=php:8.3.0RC3-fpm` |

**注意**: 当前 Dockerfile 主要针对 Debian 优化（apt-get），切换到 Alpine 可能需要调整包管理器命令。

---

## ADR-9: version_manifest.json 扁平化重构

**版本**: v0.2.0 (2026-04-27)

**问题**: 旧版 manifest 使用嵌套的 `image` + `tag` 结构，导致大量运行时格式转换散落在多个文件中：
- `version.replace('.', "")` 计算目录名
- `format!("{}:{}", image, tag)` 拼接镜像名
- `split('-')` 去后缀
- `if version.starts_with(...)` 硬编码模板选择
- `extract_image_tag()` 反向解析

**方案**: 扁平化、自描述的 manifest 结构，每条记录自带所有下游需要的信息。

**旧结构**:
```json
{
  "php": {
    "8.2": { "image": "php", "tag": "8.2-fpm", "eol": false }
  }
}
```

**新结构**:
```json
{
  "php": {
    "php82": {
      "display_name": "PHP 8.2",
      "image_tag": "php:8.2-fpm",
      "service_dir": "php82",
      "default_port": 9000,
      "show_port": false,
      "eol": false
    }
  }
}
```

**API 变更**:

| 旧 API | 新 API | 说明 |
|--------|--------|------|
| `ImageInfo` | `VersionEntry` | 结构体重命名 |
| `get_image_info()` | `get_entry()` | 按 ID 查询 |
| `get_full_image_name()` | 删除 | 直接用 `entry.image_tag` |
| `normalize_version()` | 删除 | ID 是精确的 |
| `get_merged_image_info()` | `get_merged_entry()` | 合并用户覆盖 |
| `is_version_valid()` | `is_id_valid()` | 检查 ID 是否存在 |
| `get_version_warning()` | `get_entry_warning()` | EOL 警告 |
| — | `find_entry_by_env_prefix()` | 按 env 前缀反查 |
| — | `get_available_entries()` | 返回排序后的条目列表 |

**消除的转换逻辑**:
- `version.replace('.', "")` → 直接用 `entry.service_dir`
- `format!("{}:{}", image, tag)` → 直接用 `entry.image_tag`
- `if version.starts_with(...)` → `resolve_template_dir()` 辅助函数
- `extract_image_tag()` → 删除（`load_existing_config` 使用 `find_entry_by_env_prefix` 反查）
- `managed_keys()` → 删除（`apply()` 始终生成全新 .env）

**设计原则**:
1. **数据自描述**: manifest 中每条记录包含所有下游需要的字段
2. **ID 即目录名**: manifest key（如 `php82`）与 `services/` 目录名一致
3. **向后兼容**: 生成的 `.env` 和 `docker-compose.yml` 格式不变
4. **最小改动面**: 仅修改数据结构和转换逻辑，不改动 Dockerfile、镜像源管理、备份恢复

---

↩ [返回主架构文档](./ARCHITECTURE.md)
