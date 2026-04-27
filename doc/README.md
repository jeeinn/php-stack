# PHP-Stack 文档中心

> **版本**: v0.2.0  
> **最后更新**: 2026-04-27

本文档中心整理了 PHP-Stack 项目的所有技术文档，按类别组织以便于查找和维护。

---

## 📁 文档目录结构

```
doc/
├── README.md                    # 📖 本文档 - 文档中心索引
├── architecture/                # 🏗️ 架构设计文档（稳定）
│   ├── ARCHITECTURE.md         # 系统架构
│   ├── WORKFLOWS.md            # 核心工作流程
│   ├── LOGGING.md              # 日志系统
│   ├── DECISIONS.md            # 技术决策 (ADR)
│   └── EXTENSION_GUIDE.md      # 扩展指南
├── guides/                      # 📖 使用指南（活跃）
│   ├── MIRROR_GUIDE.md         # 镜像源配置指南
│   ├── QUICK_REFERENCE.md      # 快速参考
│   ├── TESTING_GUIDE.md        # 测试规范指南
│   ├── TEST_RESTRUCTURE.md     # 测试重构说明
│   └── TESTING_QUICK_REF.md    # 测试快速参考
├── history/                     # 📜 历史归档（所有其他文档）
│   ├── YYYY-MM-DD_*.md         # 按日期归档的文档
│   ├── Dev.log.md              # 开发日志
│   ├── FIX_*.md                # 问题修复记录
│   └── ...                     # 其他历史文档
└── implementation/              # 💻 临时实现文档（功能完成后归档到 history/）
    └── .gitkeep                # 保持目录存在
```

**核心原则**:
- **architecture/** - 系统架构设计，长期稳定的核心文档
- **guides/** - 用户使用指南和开发者教程，持续更新的活跃文档
- **history/** - 所有实施总结、测试报告、修复记录等历史文档（带日期前缀）该文件夹下的文档不需要更新
- **implementation/** - 仅在功能开发过程中临时存放文档，完成后统一归档到 history/

---

## 🎯 快速导航

### 新用户入门
1. 阅读根目录 [README.md](../README.md) 了解项目概况
2. 查看 [guides/MIRROR_GUIDE.md](guides/MIRROR_GUIDE.md) 学习镜像源配置
3. 参考 [guides/QUICK_REFERENCE.md](guides/QUICK_REFERENCE.md) 快速上手
4. 阅读 [guides/TESTING_QUICK_REF.md](guides/TESTING_QUICK_REF.md) 了解测试命令

### 开发者必读
1. [architecture/ARCHITECTURE.md](architecture/ARCHITECTURE.md) - 理解系统架构
2. [guides/TESTING_GUIDE.md](guides/TESTING_GUIDE.md) - 掌握测试规范 ⭐️新增
3. 根目录 [AGENTS.md](../AGENTS.md) - AI Agent 开发指南

### 版本信息
- 根目录 [CHANGELOG.md](../CHANGELOG.md) - 版本变更历史

---

## 📝 文档维护规范

### 文档分类原则

| 文档类型 | 存放位置 | 说明 |
|---------|---------|------|
| 架构设计 | `doc/architecture/` | 系统架构、工作流程、技术决策（长期稳定） |
| 使用指南 | `doc/guides/` | 用户手册、快速参考、测试指南（持续更新） |
| 历史归档 | `doc/history/` | 实施总结、测试报告、修复记录等（带日期前缀 YYYY-MM-DD_） |
| 临时实现 | `doc/implementation/` | **仅**在功能开发过程中临时存放，完成后立即归档到 history/ |
| 项目说明 | 根目录 | README.md, CHANGELOG.md |
| 开发指南 | 根目录 | AGENTS.md |

### 文档命名规范

- **架构/指南文档**: 使用大写字母和下划线，如 `ARCHITECTURE.md`, `TESTING_GUIDE.md`
- **历史归档文档**: 必须使用日期前缀 `YYYY-MM-DD_标题.md`，便于按时间排序和追溯
- **临时实现文档**: 开发过程中可使用简洁名称，归档时必须添加日期前缀

### 文档生命周期

1. **开发阶段**: 新功能开发时，相关文档可临时存放在 `doc/implementation/`
2. **功能完成**: 功能实现后，立即将文档移动到 `doc/history/` 并添加日期前缀
3. **日常维护**: 架构文档和使用指南直接在原位置更新
4. **过时文档**: 不再适用的文档移至 `doc/history/` 并标注状态
5. **重复文档**: 合并内容后删除冗余版本
6. **更新索引**: 重要文档变更后，检查是否需要更新本文档

### implementation/ 目录使用说明

**重要**: `doc/implementation/` 目录**仅用于**功能开发过程中的临时文档存放。

**使用流程**:
```
1. 开始新功能开发 → 在 implementation/ 中创建文档
2. 功能开发中     → 持续更新 implementation/ 中的文档
3. 功能完成后     → 立即移动到 history/ 并添加日期前缀
4. 清理           → implementation/ 目录应保持为空（除 .gitkeep）
```

**示例**:
```bash
# 开发过程中
doc/implementation/FEATURE_DESIGN.md

# 功能完成后
Move-Item doc/implementation/FEATURE_DESIGN.md doc/history/2026-04-27_FEATURE_DESIGN.md
```

---

## 🔗 相关链接

- [项目主页](../README.md)
- [AI Agent 指南](../AGENTS.md)
- [变更日志](../CHANGELOG.md)
- [GitHub 仓库](https://github.com/your-repo/php-stack)

---

**维护者**: PHP-Stack Team  
**文档版本**: 1.0
