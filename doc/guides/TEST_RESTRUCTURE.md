# 测试结构重构说明

## 📋 概述

本次重构将项目的测试用例按照行业最佳实践进行了统一组织，分别规范了Rust后端和Vue前端的测试目录结构。

## 🔄 变更内容

### Rust 后端测试

#### 之前的状态
- 所有测试都分散在各个源文件中（使用 `#[cfg(test)]`）
- `tests/` 目录存在但为空

#### 现在的结构
```
src-tauri/
├── src/                          # 源代码（包含单元测试）
│   ├── engine/
│   │   ├── env_parser.rs         # #[cfg(test)] 单元测试
│   │   ├── backup_manifest.rs    # #[cfg(test)] 单元测试
│   │   ├── config_generator.rs   # #[cfg(test)] 单元测试
│   │   └── ...
│   └── docker/
│       └── tests.rs              # Docker 单元测试
└── tests/                        # 集成测试
    └── integration/
        ├── backup_restore_integration.rs      # 备份恢复集成测试
        └── config_generation_integration.rs   # 配置生成集成测试
```

**优势：**
- ✅ 单元测试靠近源代码，便于维护
- ✅ 集成测试独立组织，便于端到端测试
- ✅ 符合Rust社区标准实践

### Vue 前端测试

#### 之前的状态
- 没有任何测试框架
- 没有测试文件

#### 现在的结构
```
src/
├── components/
│   ├── __tests__/                # 组件测试
│   │   ├── EnvConfigPage.spec.ts
│   │   ├── MirrorPanel.spec.ts
│   │   ├── BackupPage.spec.ts (待创建)
│   │   └── RestorePage.spec.ts (待创建)
│   └── *.vue
├── composables/
│   ├── __tests__/                # Composable 测试
│   │   ├── useToast.spec.ts
│   │   └── useConfirmDialog.spec.ts (待创建)
│   └── *.ts
├── utils/
│   ├── __tests__/                # 工具函数测试
│   │   └── portChecker.spec.ts
│   └── *.ts
└── test/
    └── setup.ts                  # 测试全局设置
```

**新增依赖：**
- `vitest` - 测试运行器
- `@vue/test-utils` - Vue组件测试工具
- `jsdom` - DOM环境模拟
- `@vitest/ui` - 测试UI界面

**优势：**
- ✅ 测试文件与被测代码邻近，易于查找
- ✅ 统一的命名规范（`.spec.ts`）
- ✅ 支持监视模式和UI界面
- ✅ 可生成覆盖率报告

## 🚀 如何使用

### 运行Rust测试

```bash
# 运行所有测试
cargo test

# 仅运行单元测试
cargo test --lib

# 仅运行集成测试
cargo test --test '*'

# 运行特定测试
cargo test test_parse_basic_key_value

# 显示测试输出
cargo test -- --nocapture
```

### 运行Vue测试

```bash
# 监视模式（开发时使用）
npm run test

# 运行一次并退出
npm run test:run

# 打开测试UI界面
npm run test:ui

# 生成覆盖率报告
npm run test:coverage
```

## 📝 编写新测试

### Rust单元测试示例

在源文件末尾添加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_your_function() {
        // Arrange
        let input = "test";
        
        // Act
        let result = your_function(input);
        
        // Assert
        assert_eq!(result, expected_value);
    }
}
```

### Rust集成测试示例

在 `tests/integration/` 下创建新文件：

```rust
use app_lib::engine::your_module;
use tempfile::TempDir;

#[tokio::test]
async fn test_integration_workflow() {
    let temp_dir = TempDir::new().unwrap();
    // 测试逻辑...
}
```

### Vue组件测试示例

在对应的 `__tests__/` 目录下创建：

```typescript
import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import YourComponent from '../YourComponent.vue'

describe('YourComponent', () => {
  it('renders correctly', () => {
    const wrapper = mount(YourComponent)
    expect(wrapper.exists()).toBe(true)
  })
})
```

## 📊 测试覆盖目标

- **Rust核心逻辑**: ≥ 80% 行覆盖率
- **Vue关键组件**: ≥ 70% 覆盖率
- **工具函数**: 100% 覆盖率
- **Composables**: ≥ 80% 覆盖率

## 📚 详细文档

完整的测试规范和最佳实践请参考：
- [TESTING_GUIDE.md](../doc/guides/TESTING_GUIDE.md) - 测试规范指南
- [AGENTS.md](../AGENTS.md) - AI Agent开发指南（已更新测试部分）

## ⚠️ 注意事项

1. **TypeScript类型错误**：某些测试文件可能显示类型错误（如找不到`.vue`模块），这是正常的，Vitest可以正确处理
2. **Tauri API Mock**：前端测试中已Mock了Tauri API，无需真实Docker环境
3. **临时文件清理**：集成测试使用`tempfile`自动清理临时目录

## 🎯 下一步工作

建议后续补充以下测试：
- [ ] BackupPage 组件测试
- [ ] RestorePage 组件测试
- [ ] useConfirmDialog composable 测试
- [ ] 更多边界条件和错误处理测试
- [ ] 性能测试（如需要）

---

**更新日期**: 2026-04-23  
**版本**: v0.1.0
