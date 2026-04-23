# 镜像源配置列表化重构 (v0.1.0)

## 📋 重构目标

将镜像源配置从"预设方案"模式重构为类似"软件设置"的**列表形式**，每个类别下有多个可选的镜像源选项，用户可以直接选择某个选项。

## 🎯 核心变化

### 1. 数据结构变化

#### 旧结构（预设方案）
```json
{
  "presets": [
    {
      "id": "aliyun",
      "name": "阿里云全套",
      "docker_registry": "...",
      "apt": "...",
      "composer": "...",
      "npm": "..."
    }
  ],
  "categories": [...]
}
```

#### 新结构（列表形式）
```json
{
  "docker_registry": [
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
  ],
  "apt": [...],
  "composer": [...],
  "npm": [...],
  "github_proxy": [...]
}
```

### 2. 与版本管理的对比

| 特性 | 版本管理 (version_manifest.json) | 镜像源管理 (mirror_config.json) |
|------|----------------------------------|---------------------------------|
| 顶层键 | 服务类型 (php, mysql, redis, nginx) | 镜像源类别 (docker_registry, apt, composer, npm, github_proxy) |
| 第二层键 | 版本号 (8.5, 8.4, ...) | 选项 ID (aliyun, tsinghua, ...) |
| 数据结构 | `{"version": {"image": "...", "tag": "..."}}` | `[{"id": "...", "name": "...", "value": "..."}]` |
| 用户自定义 | `.user_version_overrides.json` | `.user_mirror_config.json` |
| 合并逻辑 | 默认版本 + 用户覆盖 | 默认选项 + 用户选择 |

### 3. 前端展示方式

#### 旧方式（预设方案）
```
┌─────────────────────────────┐
│ 选择预设方案:                │
│ ○ 阿里云全套                 │
│ ○ 清华大学全套               │
│ ○ 腾讯云全套                 │
└─────────────────────────────┘
```

#### 新方式（列表形式 - 类似软件设置）
```
Docker Registry:
┌──────────────────────────────────────────┐
│ ○ 阿里云        registry.cn-hangzhou...  │
│ ● 清华大学      docker.mirrors.tuna...   │ ← 当前选中
│ ○ 腾讯云        mirror.ccs.tencent...    │
│ ○ 中科大        docker.mirrors.ustc...   │
│ ○ 官方默认                               │
└──────────────────────────────────────────┘

APT 镜像:
┌──────────────────────────────────────────┐
│ ○ 阿里云        mirrors.aliyun.com       │
│ ● 清华大学      mirrors.tuna.tsinghua... │ ← 当前选中
│ ○ 腾讯云        mirrors.cloud.tencent... │
│ ...                                      │
└──────────────────────────────────────────┘
```

## 🔧 技术实现

### 1. 配置文件 (mirror_config.json)

**位置**: `src-tauri/services/mirror_config.json`

**结构**:
```json
{
  "<category_id>": [
    {
      "id": "<option_id>",
      "name": "<显示名称>",
      "value": "<镜像源地址>",
      "description": "<描述>"
    }
  ]
}
```

**支持的类别**:
- `docker_registry`: Docker 镜像仓库
- `apt`: APT 包管理器镜像
- `composer`: PHP Composer 镜像
- `npm`: Node.js NPM 镜像
- `github_proxy`: GitHub 代理

### 2. 数据结构 (mirror_config_manager.rs)

#### MirrorSourceOption
单个镜像源选项：
```rust
pub struct MirrorSourceOption {
    pub id: String,           // 选项ID (如 "aliyun")
    pub name: String,         // 显示名称 (如 "阿里云")
    pub value: String,        // 镜像源地址
    pub description: String,  // 描述
}
```

#### MergedMirrorCategory
合并后的类别信息：
```rust
pub struct MergedMirrorCategory {
    pub category_id: String,              // 类别ID
    pub options: Vec<MirrorSourceOption>, // 所有选项
    pub selected_id: String,              // 当前选中的选项ID
    pub current_value: String,            // 当前值
    pub has_user_override: bool,          // 是否有用户自定义
}
```

### 3. 核心方法

#### get_merged_mirror_list()
获取合并后的镜像源列表：
1. 加载 `mirror_config.json` 默认配置
2. 加载 `.user_mirror_config.json` 用户配置
3. 对每个类别：
   - 解析选项列表
   - 检查用户是否有自定义配置
   - 如果有，查找匹配的选项 ID
   - 如果没有，使用第一个选项作为默认
4. 返回合并结果

#### save_selected_option()
保存用户选择的选项：
1. 根据 category_id 和 option_id 查找对应的 value
2. 调用 `save_user_category()` 保存为用户自定义配置
3. 写入 `.user_mirror_config.json`

### 4. Tauri Commands

新增/修改的命令：

1. **get_merged_mirror_list()** → `Vec<MergedMirrorCategory>`
   - 返回所有类别及其选项列表
   - 包含当前选中状态

2. **save_selected_mirror_option(category_id, option_id)** → `()`
   - 保存用户选择的选项
   - 自动查找对应的 value 并保存

保留的命令：
- `save_user_mirror_category()` - 保存自定义地址
- `remove_user_mirror_category()` - 删除自定义
- `reset_all_mirror_overrides()` - 重置所有

移除的命令：
- ~~`get_merged_presets()`~~ - 不再需要预设列表
- ~~`save_selected_preset()`~~ - 改为按类别选择

## 📊 数据流示例

### 场景 1: 用户选择预设选项
```
1. 用户在 Docker Registry 类别中选择"清华大学"
   ↓
2. 前端调用 save_selected_mirror_option("docker_registry", "tsinghua")
   ↓
3. 后端查找 tsinghua 的 value: "https://docker.mirrors.tuna.tsinghua.edu.cn"
   ↓
4. 保存到 .user_mirror_config.json:
   {
     "categories": {
       "docker_registry": {
         "source": "https://docker.mirrors.tuna.tsinghua.edu.cn",
         "enabled": true
       }
     }
   }
   ↓
5. 同时更新 .env:
   DOCKER_REGISTRY_MIRROR=https://docker.mirrors.tuna.tsinghua.edu.cn
```

### 场景 2: 前端展示列表
```
1. 前端调用 get_merged_mirror_list()
   ↓
2. 后端返回:
   [
     {
       "category_id": "docker_registry",
       "options": [
         {"id": "aliyun", "name": "阿里云", "value": "..."},
         {"id": "tsinghua", "name": "清华大学", "value": "..."},
         ...
       ],
       "selected_id": "tsinghua",  // 用户选择的
       "current_value": "https://docker.mirrors.tuna.tsinghua.edu.cn",
       "has_user_override": true
     },
     ...
   ]
   ↓
3. 前端渲染为单选列表，标记 selected_id 的选项
```

### 场景 3: 用户输入自定义地址
```
1. 用户在文本框中输入自定义地址
   ↓
2. 前端调用 save_user_mirror_category("npm", "https://custom.com", "我的镜像")
   ↓
3. 保存到 .user_mirror_config.json:
   {
     "categories": {
       "npm": {
         "source": "https://custom.com",
         "enabled": true,
         "description": "我的镜像"
       }
     }
   }
   ↓
4. 前端在列表中显示"自定义"选项被选中
```

## ✅ 优势

### 1. 用户体验
- ✅ 更直观的列表选择方式
- ✅ 每个类别独立选择，灵活性更高
- ✅ 可以看到所有可用选项
- ✅ 支持自定义地址

### 2. 可维护性
- ✅ 配置外部化，易于添加新选项
- ✅ 结构与版本管理一致，降低认知负担
- ✅ 代码逻辑清晰，易于扩展

### 3. 灵活性
- ✅ 用户可以混合搭配不同类别的镜像源
- ✅ 支持细粒度的自定义
- ✅ 不限制必须使用预设组合

## 🔄 迁移指南

### 对于已有用户
如果用户之前使用了预设方案，他们的 `.env` 文件仍然有效。新的系统会：
1. 读取 `.env` 中的当前配置
2. 尝试匹配到最近的选项
3. 如果找不到匹配，标记为"自定义"

### 对于开发者
- 添加新的镜像源选项：编辑 `mirror_config.json`
- 添加新的类别：
  1. 在 `mirror_config.json` 中添加新类别数组
  2. 在 `get_category_ids()` 中添加类别 ID
  3. 在前端添加对应的 UI

## 📝 后续工作

### 前端重构（待实施）
1. 修改 `MirrorPanel.vue` 为列表形式
2. 每个类别显示为独立的单选列表
3. 高亮显示当前选中的选项
4. 添加"自定义"输入框
5. 显示选项的描述信息

### 测试
- ✅ Rust 单元测试通过（3/3）
- ⏳ 前端集成测试（待实施）
- ⏳ 端到端测试（待实施）

## 🎯 总结

本次重构将镜像源管理从"预设方案"模式转变为"列表选择"模式，与版本管理的实现方式保持一致：

```
mirror_config.json (默认配置)
    ↓
.user_mirror_config.json (用户自定义)
    ↓
MirrorConfigManager (合并逻辑)
    ↓
前端列表展示 (类似软件设置)
    ↓
生成 .env (应用配置)
```

这种设计提供了更好的灵活性和用户体验，同时保持了代码的一致性和可维护性。

---

**实施日期**: 2026-04-21  
**涉及文件**:
- `src-tauri/services/mirror_config.json` (重构)
- `src-tauri/src/engine/mirror_config_manager.rs` (重构)
- `src-tauri/src/commands.rs` (修改)
- `src-tauri/src/lib.rs` (修改)
