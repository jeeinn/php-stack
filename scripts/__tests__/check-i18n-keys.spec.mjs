// @vitest-environment node
/**
 * check-i18n-keys.mjs 的单元测试。
 *
 * 核心是防回归，分两批问题：
 *
 * 一、重复 key（envConfig 下曾出现两个同名 "toast" 节点，JSON.parse 静默保留
 * 后者，导致 applySuccess / backedUp / startSuccess 等 8 条文案整体消失，
 * 界面直接显示裸 key "envConfig.toast.applySuccess"）。
 *
 * 二、「死文案」误报。脚本一度报告 85 条未被引用的文案，逐一核查后 70 条是误报，
 * 真死文案只有 12 条 —— 若照单全删，会把仪表盘全部 toast、备份/恢复进度步骤、
 * 时区下拉框、日志级别切换器等直接打断。四类根因各自对应下面一个 describe：
 *   a. 扫描范围只有 src/，漏掉 src-tauri/src（Rust 引擎会把 i18n key 当事件载荷发出）
 *   b. 只认 t('literal')，漏掉「key 当数据传递」的引用（addLogKey、labelKey、step…）
 *   c. 子串匹配：common.select 被 common.selectPlaceholder 掩盖
 *   d. 模板字符串动态拼 key：t(`about.level.${level}`) 下的整棵子树不可静态枚举
 *
 * 三、硬编码中文：envConfig.{redis,nginx}.empty 与 software.toast.copyTooltip 三条 key
 * 之所以「死」，根因是模板/属性里把文案直接写成了中文。死文案是症状，硬编码才是病根。
 *
 * 四类断言：
 *  1. 解析层：scanDuplicateKeys 能在 JSON.parse 之前抓出同层级重复 key，且给出祖先路径
 *  2. 工具层：extractUsedKeys / extractKeyLikeLiterals / extractDynamicKeyPrefixes /
 *     stripComments / findHardcodedCjk 各自的语义与边界
 *  3. 契约层：真实语言包无重复 key、zh-CN/en key 集合一致、源码引用的 key 全部存在
 *  4. 误报回归：四类根因对应的真实 key 不得再被报成死文案
 */

import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';

import {
  LOCALE_FILES,
  scanDuplicateKeys,
  collectLeafPaths,
  extractUsedKeys,
  extractKeyLikeLiterals,
  extractDynamicKeyPrefixes,
  stripComments,
  findHardcodedCjk,
  isLikelyFileName,
  isLikelyNamespaceRef,
  diffKeys,
  collectSourceFiles,
  runCheck,
} from '../check-i18n-keys.mjs';

describe('scanDuplicateKeys', () => {
  it('抓出同父节点下的重复 key，并给出祖先路径', () => {
    // 必须手写 JSON 文本：JSON.stringify(obj) 在构造对象阶段就已经把重复 key 合并了，
    // 拿它当输入永远测不出问题
    const raw = `{
  "envConfig": {
    "toast": { "applySuccess": "a" },
    "pullConfirm": { "title": "t" },
    "toast": { "pullAllSuccess": "b" }
  }
}`;

    const dups = scanDuplicateKeys(raw);
    expect(dups).toHaveLength(1);
    expect(dups[0].key).toBe('toast');
    expect(dups[0].path).toEqual(['envConfig']);
  });

  it('不同父节点下的同名 key 不算重复', () => {
    const raw = JSON.stringify(
      {
        a: { toast: { x: 1 } },
        b: { toast: { y: 2 } },
      },
      null,
      2,
    );
    expect(scanDuplicateKeys(raw)).toEqual([]);
  });

  it('值里出现冒号不会被误判成 key', () => {
    const raw = '{"a": "时间 12:30", "a2": {"b": "x: y"}}';
    expect(scanDuplicateKeys(raw)).toEqual([]);
  });
});

describe('extractUsedKeys', () => {
  it('提取 t() / $t() 的静态字面量 key', () => {
    const src = `
      t('envConfig.toast.applySuccess', { location: x });
      $t("common.cancel");
      t('envConfig.toast.backedUp', { count: 1, files: 'a' });
    `;
    const keys = extractUsedKeys(src);
    expect(keys.has('envConfig.toast.applySuccess')).toBe(true);
    expect(keys.has('common.cancel')).toBe(true);
    expect(keys.has('envConfig.toast.backedUp')).toBe(true);
  });

  it('动态 key（模板字符串/变量）不提取，避免误报', () => {
    const src = "t(`envConfig.${name}`); t(key);";
    expect([...extractUsedKeys(src)]).toEqual([]);
  });
});

describe('extractKeyLikeLiterals（误报根因 b：key 当数据传递）', () => {
  const namespaces = new Set(['common', 'dashboard', 'backup', 'envConfig', 'workspace']);

  it('把不走 t() 的引用也算作在用', () => {
    const src = `
      addLogKey('dashboard.toast.dockerRestored');
      progress.value = { step: 'backup.progress.steps.done', percentage: 100 };
      const tz = { value: 'utc', labelKey: 'envConfig.timezone.utc' };
    `;
    const keys = extractKeyLikeLiterals(src, namespaces);
    expect(keys.has('dashboard.toast.dockerRestored')).toBe(true);
    expect(keys.has('backup.progress.steps.done')).toBe(true);
    expect(keys.has('envConfig.timezone.utc')).toBe(true);
  });

  it('误报根因 c：不产生子串 key', () => {
    // 真实事故：common.select 一度因为 common.selectPlaceholder 的存在被认定「在用」，
    // 从而漏掉一条真死文案
    const keys = extractKeyLikeLiterals("t('common.selectPlaceholder')", namespaces);
    expect(keys.has('common.selectPlaceholder')).toBe(true);
    expect(keys.has('common.select')).toBe(false);
  });

  it('首段不是语言包命名空间的点分路径不算 key', () => {
    const keys = extractKeyLikeLiterals(`x = 'v0.3.1'; y = 'data.json'; z = 'a.b.c';`, namespaces);
    expect([...keys]).toEqual([]);
  });

  it('模板字符串不参与（可能带插值，交由 extractDynamicKeyPrefixes 处理）', () => {
    expect([...extractKeyLikeLiterals('t(`about.level.${level}`)', namespaces)]).toEqual([]);
  });
});

describe('extractDynamicKeyPrefixes（误报根因 d：模板字符串动态拼 key）', () => {
  it('取出前缀，供调用方把整棵子树视为在用', () => {
    expect([...extractDynamicKeyPrefixes('{{ $t(`about.level.${level}`) }}')]).toEqual([
      'about.level.',
    ]);
  });

  it('纯静态模板串与变量调用都不产出前缀', () => {
    expect([...extractDynamicKeyPrefixes('t(`common.cancel`)')]).toEqual([]);
    expect([...extractDynamicKeyPrefixes('t(labelKey)')]).toEqual([]);
  });
});

describe('stripComments', () => {
  const ns = new Set(['common', 'dashboard', 'envConfig']);

  it('剥掉注释里作为示例出现的 key', () => {
    // 真实事故：VersionHelpModal.spec.ts 的说明注释里写了
    // "任何缺失的 key 都会原样输出 \"envConfig.versionHelp.xxx\""，
    // 该示例被当成引用后，检查报了一个根本不存在的 key
    const src = [
      '// 例如 "envConfig.versionHelp.xxx" 会原样输出',
      "/* t('dashboard.toast.nope') */",
      "t('common.cancel');",
    ].join('\n');
    expect([...extractKeyLikeLiterals(stripComments(src), ns)]).toEqual(['common.cancel']);
  });

  it('不破坏字符串里的 URL 协议', () => {
    const src = "const u = 'https://registry.npmmirror.com';";
    expect(stripComments(src)).toContain('https://registry.npmmirror.com');
  });

  it('能剥掉 Rust 的 /// 文档注释', () => {
    expect(stripComments('/// 用户级配置目录（workspace.json 等）。')).not.toContain('workspace.json');
  });
});

describe('missing 报告的噪声过滤', () => {
  it('文件名不当作 key', () => {
    expect(isLikelyFileName('backup.zip')).toBe(true);
    expect(isLikelyFileName('workspace.json')).toBe(true);
    expect(isLikelyFileName('backup.progress.steps.done')).toBe(false);
  });

  it('命名空间引用不当作 key', () => {
    const all = new Set(['envConfig.versionHelp.title', 'envConfig.versionHelp.subtitle']);
    // 测试里 getByPath(zhCN, 'envConfig.versionHelp') 传的是子树路径
    expect(isLikelyNamespaceRef('envConfig.versionHelp', all)).toBe(true);
    expect(isLikelyNamespaceRef('envConfig.versionHelpTitle', all)).toBe(false);
  });
});

describe('findHardcodedCjk（硬编码中文：死文案的病根）', () => {
  it('抓出 <template> 里的硬编码中文并给出文件绝对行号', () => {
    const src = [
      '<script>',
      "const a = '中文在 script 里不算';",
      '</script>',
      '<template>',
      '  <p>点击添加</p>',
      '</template>',
    ].join('\n');
    const hits = findHardcodedCjk(src);
    expect(hits).toHaveLength(1);
    expect(hits[0].line).toBe(5);
    expect(hits[0].text).toContain('点击添加');
  });

  it('跳过模板注释行', () => {
    const src = "<template>\n  <!-- Nginx 配置提示 -->\n  <p>{{ $t('a.b') }}</p>\n</template>";
    expect(findHardcodedCjk(src)).toEqual([]);
  });

  it('不看 script 段（中文绝大多数是代码注释）', () => {
    const src = '<script>\n// 这里是中文注释\n</script>\n<template><p>{{ x }}</p></template>';
    expect(findHardcodedCjk(src)).toEqual([]);
  });
});

describe('diffKeys', () => {
  it('给出双向差集', () => {
    const r = diffKeys(new Set(['a', 'b']), new Set(['b', 'c']));
    expect(r.onlyInA).toEqual(['a']);
    expect(r.onlyInB).toEqual(['c']);
  });
});

describe('真实语言包契约', () => {
  it('zh-CN / en 均无同层级重复 key', () => {
    for (const [name, file] of Object.entries(LOCALE_FILES)) {
      expect(scanDuplicateKeys(readFileSync(file, 'utf8')), `${name} 存在重复 key`).toEqual([]);
    }
  });

  it('zh-CN 与 en 的 key 集合完全一致', () => {
    const zh = collectLeafPaths(JSON.parse(readFileSync(LOCALE_FILES['zh-CN'], 'utf8')));
    const en = collectLeafPaths(JSON.parse(readFileSync(LOCALE_FILES.en, 'utf8')));
    const { onlyInA, onlyInB } = diffKeys(zh, en);
    expect(onlyInA).toEqual([]);
    expect(onlyInB).toEqual([]);
    expect(zh.size).toBeGreaterThan(0);
  });

  it('源码引用的每个 key 在两个语言包里都存在', () => {
    const zh = collectLeafPaths(JSON.parse(readFileSync(LOCALE_FILES['zh-CN'], 'utf8')));
    const en = collectLeafPaths(JSON.parse(readFileSync(LOCALE_FILES.en, 'utf8')));

    const used = new Set();
    expect(collectSourceFiles().length).toBeGreaterThan(0);
    for (const f of collectSourceFiles()) {
      for (const k of extractUsedKeys(readFileSync(f, 'utf8'))) used.add(k);
    }

    const missing = [...used].filter((k) => !zh.has(k) || !en.has(k));
    expect(missing).toEqual([]);
  });

  it('runCheck 整体通过', () => {
    const r = runCheck();
    expect(r.errors).toEqual([]);
    expect(r.ok).toBe(true);
  });

  it('扫描范围覆盖 src-tauri/src（Rust 引擎会直接发 i18n key）', () => {
    const files = collectSourceFiles().map((f) => f.replace(/\\/g, '/'));
    expect(files.length).toBeGreaterThan(0);
    expect(files.some((f) => f.includes('/src-tauri/src/'))).toBe(true);
    expect(files.some((f) => f.endsWith('.rs'))).toBe(true);
  });

  it('误报根因对应的真实 key 不得被报成死文案', () => {
    // 每一条都是本轮一次真实误报的现场：若照单全删，对应功能会在界面上失去文案
    // （进度条显示裸 key、时区下拉框空、日志级别按钮空白）。若某功能被正式下线
    // 导致这些 key 合法退役，请同步更新本清单。
    const mustStayUsed = [
      'dashboard.toast.dockerRestored', // b：App.vue addLogKey('dashboard.toast.…')
      'backup.progress.steps.envConfig', // a：Rust backup_engine emit_progress("…")
      'restore.progress.steps.parsing', // a：Rust restore_engine emit_progress("…")
      'workspace.reason.configuredMissing', // a：Rust paths.rs 返回的 fallback_reason
      'envConfig.timezone.utc', // b：EnvConfigPage 的 labelKey 字段
      'migration.tabs.backup', // b：MigrationPage 的 tab.labelKey
      'settings.tabs.software', // b：SettingsPage 的 tab.labelKey
      'about.level.info', // d：AboutPage $t(`about.level.${level}`)
    ];
    const r = runCheck();
    expect(mustStayUsed.filter((k) => r.unused.includes(k))).toEqual([]);
  });

  it('修正误报根因后，不应引入新的「引用但缺失」噪声', () => {
    const r = runCheck();
    expect(r.missingInZh).toEqual([]);
    expect(r.missingInEn).toEqual([]);
  });
});
