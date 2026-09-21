/**
 * i18n 语言包完整性检查。
 *
 * 注意：本文件不含 shebang。vitest 解析 ESM 时不会剥离 `#!` 行，
 * 会导致 scripts/__tests__ 下的单元测试报 "Invalid or unexpected token"。
 * 统一通过 `node scripts/check-i18n-keys.mjs` 调用（package.json 已封装为 npm scripts）。
 *
 * 背景：zh-CN.json / en.json 里曾出现「同一个父节点下写了两个 "toast"」的情况。
 * JSON.parse 对重复 key 的行为是静默取最后一个，于是 envConfig.toast 下的
 * applySuccess / backedUp / startSuccess 等 8 条文案被后写的 pull* 三条整体覆盖，
 * 运行时 t() 找不到 key，界面上直接显示 "envConfig.toast.applySuccess"。
 * 这类问题在 code review 时肉眼极难发现（两段都合法 JSON、缩进一致），
 * 因此固化为脚本检查。
 *
 * 检查三类问题（前三类 exit 1，第 4 类只提示）：
 *   1. 同一父节点下的重复 key —— 静默丢文案，最致命
 *   2. zh-CN / en 的 key 集合不一致 —— 一侧必然回退到另一种语言或裸 key
 *   3. 源码里 t('x.y') 引用的 key 在语言包里不存在 —— 界面显示裸 key
 *   4. 语言包里存在但源码从未引用的 key —— 死文案（仅提示，不阻断）
 *
 * 用法：
 *   node scripts/check-i18n-keys.mjs
 */

import { readFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { dirname, join, relative, extname } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const LOCALES_DIR = join(ROOT, 'src', 'i18n', 'locales');

export const LOCALE_FILES = {
  'zh-CN': join(LOCALES_DIR, 'zh-CN.json'),
  en: join(LOCALES_DIR, 'en.json'),
};

const SOURCE_DIRS = [join(ROOT, 'src')];
const SOURCE_EXTS = new Set(['.vue', '.ts', '.tsx', '.js', '.mjs']);
const SKIP_DIRS = new Set(['node_modules', 'dist', '.git', 'target']);

/**
 * 扫描 JSON 原文，找出同一父节点下重复出现的 key。
 * 不能用 JSON.parse —— 它在解析阶段就已经把重复 key 折叠掉了。
 *
 * @param {string} raw JSON 原文
 * @returns {Array<{ key: string, path: string[] }>} 重复项（含祖先路径，便于定位）
 */
export function scanDuplicateKeys(raw) {
  const stack = [];
  const dups = [];
  let i = 0;
  let expectKey = false;
  // 最近一次读到的对象 key；遇到 '{' 时它就是即将入栈的这一层的名字
  let pendingKey = '';

  const pathOf = () => stack.map((f) => f.name).filter(Boolean);

  while (i < raw.length) {
    const c = raw[i];

    if (c === '"') {
      let j = i + 1;
      let s = '';
      while (j < raw.length) {
        if (raw[j] === '\\') {
          s += raw[j] + raw[j + 1];
          j += 2;
          continue;
        }
        if (raw[j] === '"') break;
        s += raw[j];
        j += 1;
      }
      i = j + 1;

      // 只有后面紧跟 ':' 的字符串才是对象 key，值字符串跳过
      if (expectKey) {
        let k = i;
        while (k < raw.length && /\s/.test(raw[k])) k += 1;
        if (raw[k] === ':') {
          const frame = stack[stack.length - 1];
          pendingKey = s;
          if (frame) {
            const count = (frame.seen.get(s) || 0) + 1;
            frame.seen.set(s, count);
            if (count > 1) dups.push({ key: s, path: pathOf() });
          }
        }
      }
      continue;
    }

    if (c === '{') {
      stack.push({ name: pendingKey, seen: new Map() });
      pendingKey = '';
      expectKey = true;
      i += 1;
      continue;
    }
    if (c === '[') {
      stack.push({ name: '', seen: new Map() });
      pendingKey = '';
      expectKey = false;
      i += 1;
      continue;
    }
    if (c === '}' || c === ']') {
      stack.pop();
      expectKey = false;
      i += 1;
      continue;
    }
    if (c === ',') {
      expectKey = true;
      i += 1;
      continue;
    }
    if (c === ':') {
      // 冒号前的 key 名回填到当前 frame，用于输出祖先路径
      expectKey = false;
      i += 1;
      continue;
    }
    i += 1;
  }

  // 路径回填：key 自身在 dups 里，path 是它的祖先链
  return dups;
}

/**
 * 收集嵌套对象里所有叶子节点的点分路径。
 * @param {Record<string, unknown>} obj
 * @param {string} prefix
 * @returns {Set<string>}
 */
export function collectLeafPaths(obj, prefix = '') {
  const out = new Set();
  for (const [k, v] of Object.entries(obj || {})) {
    const path = prefix ? `${prefix}.${k}` : k;
    if (v && typeof v === 'object' && !Array.isArray(v)) {
      for (const p of collectLeafPaths(v, path)) out.add(p);
    } else {
      out.add(path);
    }
  }
  return out;
}

/**
 * 从源码文本里提取静态 i18n key：`t('a.b')` / `$t("a.b")` / `t('a.b', {...})`。
 * 模板字符串、变量拼接等动态 key 无法静态解析，直接跳过（不误报）。
 *
 * @param {string} source
 * @returns {Set<string>}
 */
export function extractUsedKeys(source) {
  const out = new Set();
  const re = /\$?\bt\(\s*['"]([A-Za-z0-9_.\-]+)['"]/g;
  let m;
  while ((m = re.exec(source))) out.add(m[1]);
  return out;
}

/**
 * 两个 key 集合的双向差集。
 * @param {Set<string>} a
 * @param {Set<string>} b
 * @returns {{ onlyInA: string[], onlyInB: string[] }}
 */
export function diffKeys(a, b) {
  const onlyInA = [...a].filter((k) => !b.has(k)).sort();
  const onlyInB = [...b].filter((k) => !a.has(k)).sort();
  return { onlyInA, onlyInB };
}

function walk(dir, out = []) {
  if (!existsSync(dir)) return out;
  for (const entry of readdirSync(dir)) {
    if (SKIP_DIRS.has(entry)) continue;
    const full = join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) {
      walk(full, out);
    } else if (SOURCE_EXTS.has(extname(entry))) {
      out.push(full);
    }
  }
  return out;
}

export function collectSourceFiles() {
  return SOURCE_DIRS.flatMap((d) => walk(d));
}

export function readLocale(name) {
  return readFileSync(LOCALE_FILES[name], 'utf8');
}

/**
 * 执行完整检查，返回结构化结果（不抛异常、不 exit，便于测试）。
 */
export function runCheck() {
  const messages = [];
  const errors = [];

  const parsed = {};
  const dupReport = [];

  for (const name of Object.keys(LOCALE_FILES)) {
    const raw = readLocale(name);
    const dups = scanDuplicateKeys(raw);
    if (dups.length > 0) {
      dupReport.push({ locale: name, dups });
      errors.push(`[${name}] 存在 ${dups.length} 处同层级重复 key（后者会静默覆盖前者）`);
    }
    parsed[name] = JSON.parse(raw);
  }

  const keys = {
    'zh-CN': collectLeafPaths(parsed['zh-CN']),
    en: collectLeafPaths(parsed.en),
  };

  const { onlyInA, onlyInB } = diffKeys(keys['zh-CN'], keys.en);
  if (onlyInA.length > 0) errors.push(`zh-CN 有 ${onlyInA.length} 个 key 在 en 中缺失`);
  if (onlyInB.length > 0) errors.push(`en 有 ${onlyInB.length} 个 key 在 zh-CN 中缺失`);

  const used = new Set();
  for (const file of collectSourceFiles()) {
    try {
      for (const k of extractUsedKeys(readFileSync(file, 'utf8'))) used.add(k);
    } catch {
      // 读失败的文件跳过，不阻断检查
    }
  }

  const missing = [...used].filter((k) => !keys['zh-CN'].has(k) || !keys.en.has(k)).sort();
  if (missing.length > 0) errors.push(`源码引用了 ${missing.length} 个语言包中不存在的 key`);

  const unused = [...keys['zh-CN']].filter((k) => !used.has(k)).sort();

  messages.push(`zh-CN: ${keys['zh-CN'].size} key / en: ${keys.en.size} key / 源码引用: ${used.size} key`);

  return {
    ok: errors.length === 0,
    errors,
    dupReport,
    missingInZh: [...used].filter((k) => !keys['zh-CN'].has(k)).sort(),
    missingInEn: [...used].filter((k) => !keys.en.has(k)).sort(),
    onlyInZh: onlyInA,
    onlyInEn: onlyInB,
    unused,
    messages,
  };
}

function main() {
  const r = runCheck();
  const lines = [...r.messages];

  for (const { locale, dups } of r.dupReport) {
    lines.push(`\n❌ [${locale}] 同层级重复 key：`);
    for (const d of dups) lines.push(`   - ${[...d.path, d.key].join('.')}`);
  }
  if (r.onlyInZh.length) lines.push(`\n❌ 仅 zh-CN 有（en 缺失）：\n${r.onlyInZh.map((k) => `   - ${k}`).join('\n')}`);
  if (r.onlyInEn.length) lines.push(`\n❌ 仅 en 有（zh-CN 缺失）：\n${r.onlyInEn.map((k) => `   - ${k}`).join('\n')}`);
  if (r.missingInZh.length) lines.push(`\n❌ 源码引用但 zh-CN 缺失：\n${r.missingInZh.map((k) => `   - ${k}`).join('\n')}`);
  if (r.missingInEn.length) lines.push(`\n❌ 源码引用但 en 缺失：\n${r.missingInEn.map((k) => `   - ${k}`).join('\n')}`);
  if (r.unused.length) lines.push(`\nℹ️  未被源码引用（死文案，可选清理，共 ${r.unused.length} 条）`);

  lines.push(r.ok ? '\n✅ i18n 检查通过' : `\n❌ i18n 检查失败：${r.errors.length} 类问题`);
  console.log(lines.join('\n'));
  process.exit(r.ok ? 0 : 1);
}

// 仅当作为 CLI 直接执行时跑 main（被 vitest import 时不执行）。
// 用 pathToFileURL 比对而非字符串拼接：Windows 下盘符与反斜杠会让手工拼接失配。
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
