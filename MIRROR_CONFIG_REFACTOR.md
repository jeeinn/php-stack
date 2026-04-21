# 镜像源管理架构优化 (V2.2)

## 📋 优化目标

参考"软件设置"中 `version_manifest.json` → `.user_version_overrides.json` 的实现模式，重构镜像源管理的实现路径，实现配置的分层管理和用户自定义覆盖。

## 🎯 新的实现路径

```
mirror_config.json (默认配置)
    ↓
.user_mirror_config.json (用户自定义覆盖)
    ↓
MirrorConfigManager (合并逻辑)
    ↓
前端镜像源管理列表 (展示合并结果)
    ↓
生成 .env (应用配置)
```

## ✅ 已完成的功能

### 1. 配置文件结构

#### mirror_config.json（默认配置）
**位置**: `src-tauri/services/mirror_config.json`

包含：
- **presets**: 5个预设方案（阿里云、清华、腾讯云、中科大、官方默认）
- **categories**: 5个镜像源类别（Docker Registry、APT、Composer、NPM、GitHub Proxy）

每个预设和类别都包含：
- `id`: 唯一标识符
- `name`: 显示名称
- `description`: 详细描述
- `default_value`: 默认值

#### .user_mirror_config.json（用户自定义）
**位置**: 项目根目录下的 `.user_mirror_config.json`

结构：
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

主要功能：
- `load_default_config()`: 加载默认配置
- `get_merged_mirror_list()`: 获取合并后的镜像源列表
- `get_merged_presets()`: 获取合并后的预设列表（含选中状态）
- `save_selected_preset()`: 保存用户选择的预设
- `save_user_category()`: 保存用户自定义类别
- `remove_user_category()`: 删除用户自定义类别
- `reset_all_overrides()`: 重置所有自定义配置

### 3. Tauri Commands

新增 6 个命令：

1. **get_merged_mirror_list()**: 获取合并后的镜像源列表
   - 返回: `Vec<MergedMirrorInfo>`
   - 包含: category_id, name, description, default_value, current_value, has_user_override

2. **get_merged_presets()**: 获取合并后的预设列表
   - 返回: `serde_json::Value`
   - 包含: 所有预设 + is_selected 标记

3. **save_selected_preset(preset_id)**: 保存用户选择的预设
   - 参数: preset_id (如 "aliyun")

4. **save_user_mirror_category(category_id, source, description)**: 保存自定义类别
   - 参数: category_id, source, description (可选)

5. **remove_user_mirror_category(category_id)**: 删除自定义类别
   - 参数: category_id

6. **reset_all_mirror_overrides()**: 重置所有自定义配置
   - 无参数

### 4. 数据结构

#### MergedMirrorInfo
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

#### UserMirrorConfig
```rust
pub struct UserMirrorConfig {
    pub selected_preset: Option<String>,           // 选中的预设ID
    pub categories: HashMap<String, UserMirrorCategory>, // 自定义类别
}
```

## 🔄 与旧实现的对比

### 旧实现（mirror_manager.rs）
```
硬编码预设 → 直接写入 .env
```

**问题**：
- ❌ 预设配置硬编码在代码中
- ❌ 无法追踪用户选择了哪个预设
- ❌ 无法区分默认配置和用户自定义
- ❌ 难以扩展新的预设或类别

### 新实现（mirror_config_manager.rs）
```
mirror_config.json → .user_mirror_config.json → 合并 → .env
```

**优势**：
- ✅ 预设配置外部化，易于维护
- ✅ 记录用户选择的预设
- ✅ 清晰区分默认值和用户自定义
- ✅ 支持细粒度的类别级别自定义
- ✅ 易于扩展新的预设和类别

## 📊 数据流示例

### 场景 1: 用户选择预设
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
   ...
```

### 场景 2: 用户自定义单个类别
```
1. 用户修改 NPM 镜像为自定义地址
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
```

### 场景 3: 前端展示合并结果
```
1. 前端调用 get_merged_mirror_list()
   ↓
2. 后端读取 mirror_config.json 和 .user_mirror_config.json
   ↓
3. 合并两者，返回:
   [
     {
       "category_id": "npm",
       "name": "NPM 镜像",
       "default_value": "https://registry.npmjs.org",
       "current_value": "https://custom.com",  // 用户自定义
       "has_user_override": true
     },
     ...
   ]
   ↓
4. 前端展示时，对有用户自定义的项添加特殊标记
```

## 🧪 测试验证

- ✅ Rust 单元测试全部通过（3/3）
  - test_load_default_config
  - test_user_config_save_and_load
  - test_get_merged_mirror_list

- ✅ 编译成功，无错误

## 📝 后续工作

### 前端集成（待实施）
1. 修改 `MirrorPanel.vue` 使用新的 API
2. 展示 `has_user_override` 标记
3. 显示用户自定义的描述信息
4. 添加"恢复默认"按钮

### .gitignore 配置
需要将 `.user_mirror_config.json` 添加到 `.gitignore`，类似于 `.user_version_overrides.json`

### 向后兼容
保留现有的 `mirror_manager.rs` 命令，确保旧代码仍然可用。

## 🎯 总结

本次优化实现了镜像源管理的分层架构：
- **默认配置层**: `mirror_config.json` - 系统预设
- **用户自定义层**: `.user_mirror_config.json` - 用户覆盖
- **合并展示层**: `MirrorConfigManager` - 智能合并
- **应用层**: `.env` - 最终生效配置

这种设计与版本管理的实现模式完全一致，提供了更好的可维护性和用户体验。

---

**实施日期**: 2026-04-21  
**涉及文件**:
- `src-tauri/services/mirror_config.json` (新建)
- `src-tauri/src/engine/mirror_config_manager.rs` (新建)
- `src-tauri/src/engine/mod.rs` (修改)
- `src-tauri/src/commands.rs` (修改)
- `src-tauri/src/lib.rs` (修改)
