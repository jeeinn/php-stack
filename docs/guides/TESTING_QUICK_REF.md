# 测试快速参考

## 🧪 运行测试

### Rust
```bash
cargo test                    # 所有测试
cargo test --lib              # 仅单元测试
cargo test --test '*'         # 仅集成测试
cargo test test_name          # 特定测试
```

### Vue
```bash
npm run test                  # 监视模式
npm run test:run              # 运行一次
npm run test:ui               # UI界面
npm run test:coverage         # 覆盖率报告
```

## 📁 测试位置

### Rust
- **单元测试**: `src-tauri/src/**/*.rs` (在 `#[cfg(test)]` 模块中)
- **集成测试**: `src-tauri/tests/integration/*.rs`

### Vue
- **组件测试**: `src/components/__tests__/*.spec.ts`
- **Composable测试**: `src/composables/__tests__/*.spec.ts`
- **工具函数测试**: `src/utils/__tests__/*.spec.ts`

## 📝 编写测试

### Rust 单元测试
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_function() {
        assert_eq!(my_function(input), expected);
    }
}
```

### Rust 集成测试
```rust
use app_lib::engine::my_module;
use tempfile::TempDir;

#[tokio::test]
async fn test_workflow() {
    let temp_dir = TempDir::new().unwrap();
    // 测试逻辑...
}
```

### Vue 组件测试
```typescript
import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import MyComponent from '../MyComponent.vue'

describe('MyComponent', () => {
  it('renders correctly', () => {
    const wrapper = mount(MyComponent)
    expect(wrapper.exists()).toBe(true)
  })
})
```

### Vue Composable 测试
```typescript
import { describe, it, expect } from 'vitest'
import { myComposable } from '../myComposable'

describe('myComposable', () => {
  it('works correctly', () => {
    const result = myComposable()
    expect(result).toBeDefined()
  })
})
```

## 🎯 测试命名规范

- **Rust**: `test_功能描述` (snake_case)
- **Vue**: `it('应该做某事', ...)` (中文或英文描述)

## 📊 覆盖目标

- Rust核心逻辑: ≥ 80%
- Vue关键组件: ≥ 70%
- 工具函数: 100%
- Composables: ≥ 80%

## 🔗 详细文档

- [TESTING_GUIDE.md](./TESTING_GUIDE.md) - 完整测试规范
- [TEST_RESTRUCTURE.md](./TEST_RESTRUCTURE.md) - 重构说明
