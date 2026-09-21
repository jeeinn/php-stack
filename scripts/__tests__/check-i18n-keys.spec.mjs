// @vitest-environment node
/**
 * check-i18n-keys.mjs 的单元测试。
 *
 * 核心是防回归：envConfig 下曾出现两个同名 "toast" 节点，JSON.parse 静默保留
 * 后者，导致 applySuccess / backedUp / startSuccess 等 8 条文案整体消失，
 * 界面直接显示裸 key "envConfig.toast.applySuccess"。
 *
 * 三类断言：
 *  1. 解析层：scanDuplicateKeys 能在 JSON.parse 之前抓出同层级重复 key，
 *     且能给出祖先路径（否则报告里只有一个 "toast"，无法定位）
 *  2. 契约层：真实语言包无重复 key、zh-CN/en key 集合一致、源码引用的
 *     key 全部存在
 *  3. 工具层：extractUsedKeys 只取静态字面量，动态 key 不误报
 */

import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';

import {
  LOCALE_FILES,
  scanDuplicateKeys,
  collectLeafPaths,
  extractUsedKeys,
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
});
