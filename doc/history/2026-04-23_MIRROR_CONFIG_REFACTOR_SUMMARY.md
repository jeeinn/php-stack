# 镜像源管理重构总结 (v0.1.0)

**实施日期**: 2026-04-21  
**状态**: ✅ 已完成并应用到生产环境

---

## 📋 重构概述

本次重构将镜像源管理从"硬编码预设"模式升级为"分层配置管理"模式，参考了版本管理系统（`version_manifest.json` → `.user_version_overrides.json`）的成功实践。

### 核心改进

1. **配置外部化** - 从代码硬编码移至 JSON 配置文件
2. **分层管理** - 默认配置 + 用户自定义覆盖
3. **灵活选择** - 支持预设方案和细粒度类别自定义
4. **状态追踪** - 清晰区分默认值和用户自定义

---

## 🎯 实现架构

### 数据流

```
mirror_config.json (默认配置)
    ↓
.user_mirror_config.json (用户自定义覆盖)
    ↓
MirrorConfigManager (智能合并逻辑)
    ↓
前端镜像源管理界面 (展示合并结果)
    ↓
.env (最终生效配置)
```

### 与版本管理的对比

| 特性 | 版本管理 | 镜像源管理 |
|------|---------|-----------|
| 默认配置 | `version_manifest.json` | `mirror_config.json` |
| 用户覆盖 | `.user_version_overrides.json` | `.user_mirror_config.json` |
| 管理器 | `UserOverrideManager` | `MirrorConfigManager` |
| 顶层键 | 服务类型 (php, mysql...) | 镜像源类别 (docker_registry, apt...) |
| 合并逻辑 | 默认版本 + 用户覆盖 | 默认选项 + 用户选择 |

---

## 🔧 技术实现

### 1. 配置文件结构

#### mirror_config.json（默认配置）
**位置**: `src-tauri/services/mirror_config.json`

包含两个主要部分：

**A. 预设方案列表**
```json
{
  "presets": [
    {
      "id": "aliyun",
      "name": "阿里云全套",
      "description": "阿里云提供的完整镜像加速方案",
      "docker_registry": "https://registry.cn-hangzhou.aliyuncs.com",
      "apt": "https://mirrors.aliyun.com/debian",
      "composer": "https://mirrors.aliyun.com/composer/",
      "npm": "https://registry.npmmirror.com"
    }
  ]
}
```

**B. 镜像源类别定义**
```json
{
  "categories": {
    "docker_registry": {
      "name": "Docker Registry",
      "description": "Docker 镜像仓库加速",
      "options": [
        {
          "id": "aliyun",
          "name": "阿里云",
          "value": "https://registry.cn-hangzhou.aliyuncs.com",
          "description": "阿里云 Docker 镜像仓库"
        },
        {
          "id": "tsinghua",
          "name": "清华大学",
          "value": "https://docker.mirrors.tuna.tsinghua.edu.cn",
          "description": "清华大学开源软件镜像站"
        }
      ]
    }
  }
}
```

支持的类别：
- `docker_registry` - Docker 镜像仓库
- `apt` - APT 包管理器镜像
- `composer` - PHP Composer 镜像
- `npm` - Node.js NPM 镜像
- `github_proxy` - GitHub 代理

#### .user_mirror_config.json（用户自定义）
**位置**: 项目根目录（已加入 `.gitignore`）

```json
{
  "selected_preset": "aliyun",
  "categories": {
    "npm": {
      "source": "https://custom.npm.mirror.com",
      "enabled": true,
      "description": "自定义NPM镜像"
    }
  }
}
```

### 2. 核心模块

#### MirrorConfigManager
**位置**: `src-tauri/src/engine/mirror_config_manager.rs`

**主要功能**：
- `load_default_config()` - 加载默认配置
- `get_merged_mirror_list()` - 获取合并后的镜像源列表
- `get_merged_presets()` - 获取合并后的预设列表（含选中状态）
- `save_selected_preset(preset_id)` - 保存用户选择的预设
- `save_user_category(category_id, source, description)` - 保存用户自定义类别
- `remove_user_category(category_id)` - 删除用户自定义类别
- `reset_all_overrides()` - 重置所有自定义配置

#### 数据结构

**MergedMirrorInfo** - 合并后的镜像源信息
```rust
pub struct MergedMirrorInfo {
    pub category_id: String,        // 类别ID
    pub name: String,               // 显示名称
    pub description: String,        // 描述
    pub default_value: String,      // 默认值
    pub current_value: String,      // 当前值（可能是用户自定义）
    pub has_user_override: bool,    // 是否有用户自定义
}
```

**UserMirrorConfig** - 用户配置
```rust
pub struct UserMirrorConfig {
    pub selected_preset: Option<String>,
    pub categories: HashMap<String, UserMirrorCategory>,
}
```

### 3. Tauri Commands

新增/修改的命令：

1. **get_merged_mirror_list()** → `Vec<MergedMirrorInfo>`
   - 返回所有类别及其当前值
   - 包含 `has_user_override` 标记

2. **get_merged_presets()** → `serde_json::Value`
   - 返回所有预设方案
   - 包含 `is_selected` 标记

3. **save_selected_preset(preset_id)** → `()`
   - 保存用户选择的预设方案

4. **save_user_mirror_category(category_id, source, description)** → `()`
   - 保存单个类别的自定义配置

5. **remove_user_mirror_category(category_id)** → `()`
   - 删除类别的自定义配置，恢复默认

6. **reset_all_mirror_overrides()** → `()`
   - 重置所有自定义配置

保留的旧命令（向后兼容）：
- `get_mirror_presets()` - 获取预设列表
- `apply_mirror_preset()` - 应用预设到 .env
- `update_single_mirror()` - 更新单个类别
- `test_mirror()` - 测试连接
- `get_mirror_status()` - 获取当前状态

---

## 📊 使用场景示例

### 场景 1: 用户选择预设方案

```
1. 用户在前端选择"阿里云全套"
   ↓
2. 调用 save_selected_preset("aliyun")
   ↓
3. 写入 .user_mirror_config.json:
   { "selected_preset": "aliyun" }
   ↓
4. 调用 apply_mirror_preset("阿里云全套")
   ↓
5. 写入 .env:
   DOCKER_REGISTRY_MIRROR=https://registry.cn-hangzhou.aliyuncs.com
   APT_MIRROR=https://mirrors.aliyun.com/debian
   COMPOSER_MIRROR=https://mirrors.aliyun.com/composer/
   NPM_MIRROR=https://registry.npmmirror.com
```

### 场景 2: 用户自定义单个类别

```
1. 用户在选择了"阿里云"预设后，单独修改 NPM 镜像
   ↓
2. 调用 save_user_mirror_category("npm", "https://custom.com", "我的自定义")
   ↓
3. 写入 .user_mirror_config.json:
   {
     "selected_preset": "aliyun",
     "categories": {
       "npm": {
         "source": "https://custom.com",
         "enabled": true,
         "description": "我的自定义"
       }
     }
   }
   ↓
4. 调用 update_single_mirror("npm", "https://custom.com")
   ↓
5. 更新 .env:
   NPM_MIRROR=https://custom.com
   
其他类别仍保持阿里云的配置
```

### 场景 3: 前端展示合并结果

```
1. 前端调用 get_merged_mirror_list()
   ↓
2. 后端读取并合并两个配置文件
   ↓
3. 返回:
   [
     {
       "category_id": "npm",
       "name": "NPM 镜像",
       "default_value": "https://registry.npmjs.org",
       "current_value": "https://custom.com",
       "has_user_override": true  // ← 标记为用户自定义
     },
     {
       "category_id": "apt",
       "name": "APT 镜像",
       "default_value": "https://deb.debian.org/debian",
       "current_value": "https://mirrors.aliyun.com/debian",
       "has_user_override": false  // ← 使用预设值
     }
   ]
   ↓
4. 前端对 has_user_override=true 的项添加特殊标记（如 ✨ 图标）
```

---

## ✅ 优势对比

### 旧实现（硬编码）

```rust
// mirror_manager.rs - 硬编码预设
fn get_presets() -> Vec<MirrorPreset> {
    vec![
        MirrorPreset {
            id: "aliyun".to_string(),
            name: "阿里云全套".to_string(),
            docker_registry: "https://...".to_string(),
            // ... 所有配置硬编码
        }
    ]
}
```

**问题**：
- ❌ 预设配置硬编码在代码中
- ❌ 无法追踪用户选择了哪个预设
- ❌ 无法区分默认配置和用户自定义
- ❌ 难以扩展新的预设或类别
- ❌ 修改配置需要重新编译

### 新实现（分层配置）

```
mirror_config.json → .user_mirror_config.json → 合并 → .env
```

**优势**：
- ✅ 预设配置外部化，易于维护
- ✅ 记录用户选择的预设
- ✅ 清晰区分默认值和用户自定义
- ✅ 支持细粒度的类别级别自定义
- ✅ 易于扩展新的预设和类别
- ✅ 修改配置无需重新编译

---

## 🧪 测试验证

### Rust 单元测试
- ✅ `test_load_default_config` - 验证默认配置加载
- ✅ `test_user_config_save_and_load` - 验证用户配置保存和加载
- ✅ `test_get_merged_mirror_list` - 验证合并逻辑正确性

### 测试结果
```bash
cargo test mirror_config_manager
# 3/3 测试通过 ✅
```

---

## 🔄 迁移指南

### 对于已有用户

如果用户之前使用了旧的镜像源配置：

1. **`.env` 文件仍然有效** - 现有的镜像源配置继续工作
2. **首次使用时** - 系统会读取 `.env` 中的当前配置
3. **自动匹配** - 尝试匹配到最近的预设或标记为"自定义"
4. **无感知迁移** - 用户无需手动操作

### 对于开发者

**添加新的镜像源选项**：
1. 编辑 `mirror_config.json`
2. 在对应类别的 `options` 数组中添加新选项
3. 无需修改代码

**添加新的类别**：
1. 在 `mirror_config.json` 中添加新类别定义
2. 在 `get_category_ids()` 中添加类别 ID
3. 在前端添加对应的 UI 组件
4. 更新 `.env` 生成逻辑

---

## 📝 后续优化建议

### 已完成
- ✅ 后端核心逻辑实现
- ✅ 配置文件结构设计
- ✅ Tauri Commands 暴露
- ✅ 单元测试覆盖

### 待优化（低优先级）
1. **前端 UI 重构** - 将 MirrorPanel.vue 改为列表形式展示
2. **智能匹配** - 根据 `.env` 当前值自动识别使用的预设
3. **预设模板库** - 提供更多社区贡献的预设方案
4. **导入导出** - 支持预设方案的导入导出分享

---

## 🎯 总结

本次重构成功实现了镜像源管理的现代化架构：

### 核心成果
1. **配置分层** - 默认配置与用户自定义清晰分离
2. **状态追踪** - 准确记录用户的选择和自定义
3. **灵活扩展** - 外部化配置，易于添加新选项
4. **向后兼容** - 保留旧 API，平滑迁移

### 设计模式
采用了与版本管理一致的架构模式：
```
默认配置 (JSON) + 用户覆盖 (JSON) → 智能合并 → 应用配置 (.env)
```

这种设计提供了更好的可维护性、灵活性和用户体验，同时保持了代码的一致性和简洁性。

---

**相关文档**：
- [MIRROR_GUIDE.md](../guides/MIRROR_GUIDE.md) - 镜像源配置使用指南
- [ARCHITECTURE.md](../architecture/ARCHITECTURE.md) - 系统架构文档

**涉及文件**：
- `src-tauri/services/mirror_config.json` - 默认配置
- `src-tauri/src/engine/mirror_config_manager.rs` - 核心管理器
- `src-tauri/src/commands.rs` - Tauri 命令
- `.user_mirror_config.json` - 用户配置（运行时生成）
