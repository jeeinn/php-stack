# 测试规范指南

本文档描述了 PHP-Stack 项目的测试规范和最佳实践。

## 📁 测试目录结构

### Rust 后端测试

```
src-tauri/
├── src/                          # 源代码
│   ├── engine/
│   │   ├── env_parser.rs         # 包含单元测试 (#[cfg(test)])
│   │   ├── backup_manifest.rs    # 包含单元测试
│   │   └── ...
│   └── docker/
│       └── tests.rs              # Docker 相关单元测试
└── tests/                        # 集成测试
    └── integration/
        ├── backup_restore_integration.rs  # 备份恢复集成测试
        └── config_generation_integration.rs  # 配置生成集成测试
```

**规范：**
- **单元测试**：放在源文件内的 `#[cfg(test)] mod tests { ... }` 模块中
- **集成测试**：放在 `tests/integration/` 目录下，每个功能模块一个文件
- **测试命名**：使用 `test_功能描述` 格式（snake_case）

### Vue 前端测试

```
src/
├── components/
│   ├── __tests__/                # 组件测试
│   │   ├── EnvConfigPage.spec.ts
│   │   ├── MirrorPanel.spec.ts
│   │   ├── BackupPage.spec.ts
│   │   └── RestorePage.spec.ts
│   └── *.vue
├── composables/
│   ├── __tests__/                # Composable 测试
│   │   ├── useToast.spec.ts
│   │   └── useConfirmDialog.spec.ts
│   └── *.ts
├── utils/
│   ├── __tests__/                # 工具函数测试
│   │   └── portChecker.spec.ts
│   └── *.ts
└── test/
    └── setup.ts                  # 测试全局设置
```

**规范：**
- **测试文件位置**：与被测试代码同级目录下的 `__tests__/` 文件夹
- **测试文件命名**：`{模块名}.spec.ts` 或 `{模块名}.test.ts`
- **测试框架**：Vitest + @vue/test-utils

## 🧪 运行测试

### Rust 测试

```bash
# 运行所有测试
cargo test

# 运行单元测试
cargo test --lib

# 运行集成测试
cargo test --test '*'

# 运行特定测试
cargo test test_parse_basic_key_value

# 显示测试输出
cargo test -- --nocapture
```

### Vue 测试

```bash
# 运行所有测试（监视模式）
npm run test

# 运行所有测试（一次性）
npm run test:run

# 运行测试 UI 界面
npm run test:ui

# 生成覆盖率报告
npm run test:coverage
```

## 📝 测试编写规范

### Rust 单元测试示例

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_key_value() {
        let content = "KEY=value";
        let env = EnvFile::parse(content).unwrap();
        assert_eq!(env.lines.len(), 1);
        assert_eq!(
            env.lines[0],
            EnvLine::KeyValue {
                key: "KEY".to_string(),
                value: "value".to_string(),
                inline_comment: None,
            }
        );
    }

    #[test]
    fn test_error_handling() {
        let content = "INVALID_LINE";
        let result = EnvFile::parse(content);
        assert!(result.is_err());
    }
}
```

### Rust 集成测试示例

```rust
// tests/integration/backup_restore_integration.rs
use app_lib::engine::backup_engine;
use tempfile::TempDir;

#[tokio::test]
async fn test_backup_and_restore_workflow() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let workspace_path = temp_dir.path().to_path_buf();
    
    // 创建测试环境
    let env_content = "PHP_VERSION=8.2\n";
    fs::write(workspace_path.join(".env"), env_content).unwrap();
    
    // 执行备份
    let result = backup_engine::create_backup(
        &workspace_path,
        &workspace_path.join("backup.zip"),
        false, vec![], false
    ).await;
    
    assert!(result.is_ok());
    assert!(workspace_path.join("backup.zip").exists());
}
```

### Vue 组件测试示例

```typescript
import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import EnvConfigPage from '../EnvConfigPage.vue'

describe('EnvConfigPage', () => {
  it('renders the component', () => {
    const wrapper = mount(EnvConfigPage)
    expect(wrapper.exists()).toBe(true)
  })

  it('displays service selection options', () => {
    const wrapper = mount(EnvConfigPage)
    expect(wrapper.find('.service-selector').exists()).toBe(true)
  })

  it('handles user input correctly', async () => {
    const wrapper = mount(EnvConfigPage)
    const input = wrapper.find('input[type="text"]')
    await input.setValue('8.2')
    expect(input.element.value).toBe('8.2')
  })
})
```

### Vue Composable 测试示例

```typescript
import { describe, it, expect } from 'vitest'
import { showToast, getToasts } from '../useToast'

describe('useToast', () => {
  it('shows a toast message', () => {
    showToast('Test message', 'success')
    const toasts = getToasts()
    expect(toasts.value.length).toBeGreaterThan(0)
  })
})
```

## ✅ 测试最佳实践

### 通用原则

1. **测试独立性**：每个测试应该独立运行，不依赖其他测试的状态
2. **测试可重复性**：测试结果应该是确定的，不受外部环境影响
3. **测试命名清晰**：测试名称应该清楚描述测试的内容和预期行为
4. **AAA 模式**：Arrange（准备）、Act（执行）、Assert（断言）
5. **边界条件测试**：测试正常情况、边界情况和错误情况

### Rust 特定实践

1. **单元测试 vs 集成测试**：
   - 单元测试：测试单个函数/模块的内部逻辑
   - 集成测试：测试多个模块之间的交互

2. **使用临时目录**：涉及文件操作的测试使用 `tempfile` crate

3. **异步测试**：使用 `#[tokio::test]` 标记异步测试

4. **属性测试**：对于纯函数，使用 `proptest` 进行属性测试

### Vue 特定实践

1. **组件测试策略**：
   - 测试用户可见的行为，而非实现细节
   - 优先测试交互和状态变化
   - 使用 `data-testid` 选择元素

2. **Mock 外部依赖**：
   - Mock Tauri API 调用
   - Mock 网络请求
   - Mock 定时器

3. **避免测试实现细节**：
   - ❌ 不要测试内部方法
   - ✅ 测试用户交互后的结果

## 🔍 测试覆盖目标

- **Rust 核心逻辑**：≥ 80% 行覆盖率
- **Vue 组件**：关键组件 ≥ 70% 覆盖率
- **工具函数**：100% 覆盖率
- **Composables**：≥ 80% 覆盖率

## 🚀 CI/CD 集成

未来可以在 CI/CD 流程中添加：

```yaml
# .github/workflows/test.yml
name: Test
on: [push, pull_request]
jobs:
  test-rust:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - run: cargo test --all
      
  test-vue:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - run: npm ci
      - run: npm run test:run
```

## 📚 参考资源

- [Rust Book - Writing Tests](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Vitest Documentation](https://vitest.dev/)
- [@vue/test-utils Documentation](https://test-utils.vuejs.org/)
- [Testing Library Best Practices](https://testing-library.com/docs/guiding-principles)
