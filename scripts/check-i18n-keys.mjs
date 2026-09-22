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
 * 检查五类问题（前三类 exit 1，第 4、5 类只提示）：
 *   1. 同一父节点下的重复 key —— 静默丢文案，最致命
 *   2. zh-CN / en 的 key 集合不一致 —— 一侧必然回退到另一种语言或裸 key
 *   3. 源码里引用的 key 在语言包里不存在 —— 界面显示裸 key
 *   4. 语言包里存在但源码从未引用的 key —— 死文案（仅提示，不阻断）
 *   5. <template> 里硬编码的中文 —— i18n 漏网（仅提示，不阻断）
 *
 * 「是否被引用」的判定曾有四类误报根因，均已修正（详见下面对应函数）：
 *   a. 扫描范围只有 src/，漏掉 src-tauri/src —— 而 Rust 引擎会把 i18n key
 *      直接作为事件载荷发出（emit_progress("backup.progress.steps.envConfig")），
 *      这类 key 在 src/ 下确实不存在，但运行时在用。见 SOURCE_DIRS。
 *   b. 只认 t('literal') 形态，漏掉「key 当数据传递」的引用 ——
 *      addLogKey('dashboard.toast.x')、progress.value = { step: 'backup.progress.steps.done' }、
 *      { value: 'utc', labelKey: 'envConfig.timezone.utc' } 等。见 extractKeyLikeLiterals。
 *   c. 子串匹配 —— common.select 会被 common.selectPlaceholder 掩盖而误报成「在用」。
 *      extractKeyLikeLiterals 要求整段被引号包裹，天然带词边界。
 *   d. 模板字符串动态拼出的 key（t(`about.level.${level}`)）不可静态枚举。见
 *      extractDynamicKeyPrefixes：命中前缀即把整棵子树保守地视为在用，宁可不报也不误删。
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

// 误报根因 a：src-tauri/src 必须一并扫描。Rust 引擎（backup_engine / restore_engine /
// commands/paths.rs）会把 i18n key 当事件载荷或返回字段发给前端，前端再用 $t(变量) 渲染。
export const SOURCE_DIRS = [join(ROOT, 'src'), join(ROOT, 'src-tauri', 'src')];

// index.html 也可能引用 key（title / meta 等），但它不是目录，单独列出。
export const EXTRA_SOURCE_FILES = [join(ROOT, 'index.html')];

// 注意：故意不含 .json —— src/i18n/locales/*.json 自身就在 src/ 下，
// 若把 .json 计入，语言包里的每个 key 都会「自己引用自己」，死文案检查直接失效。
export const SOURCE_EXTS = new Set(['.vue', '.ts', '.tsx', '.js', '.mjs', '.rs']);
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
 * 剥离注释，避免把「文档里举例的 key」当成真实引用。
 *
 * 起因：VersionHelpModal.spec.ts 的说明注释里写了
 *   「任何缺失的 key 都会让 vue-i18n 原样输出 "envConfig.versionHelp.xxx"」
 * 注释里的示例被当成引用后，检查会报一个根本不存在的 key。
 * Rust 侧 /// 文档注释里的 workspace.json 同理。
 *
 * 取舍：这是纯文本级剥离，不解析字符串字面量，因此理论上可能切掉
 * 字符串内部出现的 `//` 之后的内容。实际影响可忽略 —— 被切掉的片段要么
 * 不含点分路径，要么本就是 URL（https://）已由 `[^:]` 保护。
 *
 * @param {string} source
 * @returns {string}
 */
export function stripComments(source) {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, ' ')
    .replace(/(^|[^:])\/\/[^\n]*/g, '$1');
}

/**
 * 从源码文本里提取 `t('a.b')` / `$t("a.b")` / `t('a.b', {...})` 形态的静态 key。
 * 模板字符串、变量拼接等动态 key 无法静态解析，直接跳过（不误报）。
 *
 * 保留此函数是为了表达「严格的 t() 字面量」这一语义；判定「是否被引用」时
 * 用的是更宽的 extractKeyLikeLiterals，见该函数注释。
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
 * 误报根因 b 的修正：把「key 当作普通字符串数据传递」的引用也算作在用。
 *
 * 这类引用不走 t()，静态上只是一段点分路径字符串，例如：
 *   addLogKey('dashboard.toast.dockerRestored')
 *   progress.value = { step: 'backup.progress.steps.done' }
 *   { value: 'utc', labelKey: 'envConfig.timezone.utc' }
 * Rust 侧同理：Self::emit_progress(app_handle, "restore.progress.steps.parsing", 5)
 *
 * 误报根因 c 的修正：整段必须被引号完整包裹，因此 common.selectPlaceholder
 * 不会产出 common.select，天然带词边界。
 *
 * 为避免把普通字符串（"v0.3.1"、"data.json"）当成 key，要求首段命中语言包的
 * 顶级命名空间。
 *
 * 注意：反引号模板串有意不匹配 —— 它可能带 ${} 插值，交给
 * extractDynamicKeyPrefixes 处理。
 *
 * @param {string} source
 * @param {Set<string>} namespaces 语言包顶级命名空间
 * @returns {Set<string>}
 */
export function extractKeyLikeLiterals(source, namespaces) {
  const out = new Set();
  const re = /['"]([A-Za-z][A-Za-z0-9_]*(?:\.[A-Za-z0-9_]+)+)['"]/g;
  let m;
  while ((m = re.exec(source))) {
    const key = m[1];
    if (namespaces.has(key.split('.')[0])) out.add(key);
  }
  return out;
}

/**
 * 误报根因 d 的修正：收集模板字符串动态拼 key 的前缀。
 *
 * t(`about.level.${level}`) → 前缀 'about.level.'
 * 调用方应把该前缀下的整棵子树视为在用：动态取值集合无法静态枚举，
 * 宁可少报一条死文案，也不能误删一条运行时可达的文案。
 *
 * @param {string} source
 * @returns {Set<string>}
 */
export function extractDynamicKeyPrefixes(source) {
  const out = new Set();
  const re = /(?<![\w.])t\(\s*`([^`]*)\$\{/g;
  let m;
  while ((m = re.exec(source))) out.add(m[1]);
  return out;
}

/**
 * 常见的文件扩展名。用于排除「看起来像 key、其实是文件名」的字符串，
 * 例如 backup.zip、workspace.json —— 它们的首段恰好命中 backup / workspace
 * 这两个顶级命名空间，只靠命名空间过滤挡不住。
 */
export const FILE_EXT_SEGMENTS = new Set([
  'zip', 'tar', 'gz', 'tgz', 'bz2', 'xz', '7z',
  'json', 'json5', 'yml', 'yaml', 'toml', 'ini', 'cnf', 'conf', 'cfg', 'env',
  'ts', 'tsx', 'js', 'mjs', 'cjs', 'mts', 'cts', 'vue', 'rs', 'go', 'py', 'rb',
  'java', 'kt', 'swift', 'c', 'h', 'cc', 'cpp', 'hpp', 'cs', 'php', 'sh', 'ps1', 'cmd', 'bat',
  'md', 'mdx', 'txt', 'rst', 'log', 'lock', 'snap', 'sql', 'db', 'sqlite', 'sqlite3',
  'html', 'htm', 'css', 'scss', 'sass', 'less', 'svg', 'png', 'jpg', 'jpeg', 'gif',
  'webp', 'avif', 'ico', 'bmp', 'pdf', 'csv', 'tsv', 'xlsx', 'xls', 'docx', 'doc',
  'pptx', 'ppt', 'mp3', 'mp4', 'mov', 'avi', 'webm', 'wasm', 'dll', 'so', 'dylib',
  'exe', 'bin', 'o', 'a', 'd', 'rlib', 'rmeta', 'pdb', 'map', 'bak', 'tmp', 'temp',
  'pem', 'key', 'crt', 'cer', 'p12', 'pfx', 'example', 'sample', 'dist', 'min',
]);

/**
 * 判断该字面量是否是「命名空间引用」而非某个具体文案。
 *
 * 命中场景：测试里 `getByPath(zhCN, 'envConfig.versionHelp')` —— 这里传的是
 * 子树路径，用来遍历整个 versionHelp 命名空间，本身不是一条文案。
 * 判据：它是某个真实叶子 key 的严格前缀。
 *
 * @param {string} key
 * @param {Iterable<string>} allKeys 语言包全部叶子 key
 * @returns {boolean}
 */
export function isLikelyNamespaceRef(key, allKeys) {
  const prefix = `${key}.`;
  for (const k of allKeys) {
    if (k.startsWith(prefix)) return true;
  }
  return false;
}

/**
 * 判断该字面量是否更像文件名而非 i18n key（末段是已知扩展名）。
 * @param {string} key
 * @returns {boolean}
 */
export function isLikelyFileName(key) {
  const segs = key.split('.');
  return segs.length > 1 && FILE_EXT_SEGMENTS.has(segs[segs.length - 1].toLowerCase());
}

/**
 * 第 5 类检查：找出 <template> 里硬编码的中文。
 *
 * 设计取舍：
 * - 只在 <template> 块内找。script 段的中文绝大多数是代码注释，不该报；
 *   <template> 注释（<!-- -->）也整行跳过。
 * - 只在「同一行」判定。文案被拆成多行字符串的情况会漏，但那是少数，
 *   换来实现简单、零误报。
 * - 门槛设为连续 2 个及以上汉字：单个汉字过于常见（如 i18n 里故意保留的
 *   语言自称「中文」），连续 2 个以上才基本可以断定是漏网的文案。
 *
 * 为什么需要这一类：本轮清理发现 envConfig.redis.empty / envConfig.nginx.empty /
 * software.toast.copyTooltip 三条 key 之所以成为「死文案」，根因正是模板里把文案
 * 直接写成了中文、没走 i18n —— 结果中英双语 key 还在，英文界面却显示中文。
 * 死文案是症状，硬编码才是病根。
 *
 * @param {string} source
 * @returns {Array<{ line: number, text: string }>}
 */
export function findHardcodedCjk(source) {
  const out = [];
  const m = source.match(/<template>[\s\S]*?<\/template>/);
  if (!m) return out;
  const startLine = source.slice(0, m.index).split(/\r?\n/).length;
  m[0].split(/\r?\n/).forEach((line, i) => {
    if (line.includes('<!--')) return;
    if (/[\u4e00-\u9fa5]{2,}/.test(line)) {
      out.push({ line: startLine + i, text: line.trim().slice(0, 120) });
    }
  });
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
  return [
    ...SOURCE_DIRS.flatMap((d) => walk(d)),
    ...EXTRA_SOURCE_FILES.filter((f) => existsSync(f)),
  ];
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

  const namespaces = new Set(Object.keys(parsed['zh-CN']));

  const used = new Set();
  const dynamicPrefixes = new Set();
  const hardcodedCjk = [];

  for (const file of collectSourceFiles()) {
    let src;
    try {
      src = readFileSync(file, 'utf8');
    } catch {
      // 读失败的文件跳过，不阻断检查
      continue;
    }
    // 注释里的 key 示例不算引用（详见 stripComments）
    const code = stripComments(src);
    for (const k of extractUsedKeys(code)) used.add(k);
    for (const k of extractKeyLikeLiterals(code, namespaces)) used.add(k);
    for (const p of extractDynamicKeyPrefixes(code)) dynamicPrefixes.add(p);

    if (extname(file) === '.vue') {
      const hits = findHardcodedCjk(src);
      if (hits.length > 0) {
        hardcodedCjk.push({
          file: relative(ROOT, file).replace(/\\/g, '/'),
          hits,
        });
      }
    }
  }

  // 动态前缀下的整棵子树保守地视为在用
  for (const k of [...keys['zh-CN'], ...keys.en]) {
    for (const p of dynamicPrefixes) {
      if (k.startsWith(p)) used.add(k);
    }
  }

  // 「引用了但语言包里没有」只保留真正可疑的：排除文件名与命名空间引用，
  // 否则 backup.zip / workspace.json / envConfig.versionHelp 这类字面量会持续误报。
  const isSuspectKey = (k) =>
    (!keys['zh-CN'].has(k) || !keys.en.has(k)) &&
    !isLikelyFileName(k) &&
    !isLikelyNamespaceRef(k, keys['zh-CN']);

  const missing = [...used].filter(isSuspectKey).sort();
  if (missing.length > 0) errors.push(`源码引用了 ${missing.length} 个语言包中不存在的 key`);

  const unused = [...keys['zh-CN']].filter((k) => !used.has(k)).sort();

  messages.push(
    `zh-CN: ${keys['zh-CN'].size} key / en: ${keys.en.size} key / 源码引用: ${used.size} key`,
  );
  messages.push(
    `扫描范围: ${SOURCE_DIRS.map((d) => relative(ROOT, d).replace(/\\/g, '/')).join(' + ')}` +
      ` + ${EXTRA_SOURCE_FILES.map((f) => relative(ROOT, f).replace(/\\/g, '/')).join(', ')}`,
  );
  if (dynamicPrefixes.size > 0) {
    messages.push(`动态 key 前缀（其下整棵子树视为在用）: ${[...dynamicPrefixes].join(', ')}`);
  }

  const hardcodedCount = hardcodedCjk.reduce((n, f) => n + f.hits.length, 0);

  return {
    ok: errors.length === 0,
    errors,
    dupReport,
    missingInZh: [...used].filter((k) => !keys['zh-CN'].has(k) && isSuspectKey(k)).sort(),
    missingInEn: [...used].filter((k) => !keys.en.has(k) && isSuspectKey(k)).sort(),
    onlyInZh: onlyInA,
    onlyInEn: onlyInB,
    unused,
    hardcodedCjk,
    hardcodedCount,
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
  if (r.unused.length) {
    lines.push(`\nℹ️  未被源码引用（死文案，可选清理，共 ${r.unused.length} 条）`);
    for (const k of r.unused) lines.push(`   - ${k}`);
  }
  if (r.hardcodedCount) {
    lines.push(`\nℹ️  <template> 中硬编码中文（i18n 漏网，可选清理，共 ${r.hardcodedCount} 处）`);
    for (const { file, hits } of r.hardcodedCjk) {
      for (const h of hits) lines.push(`   - ${file}:${h.line}  ${h.text}`);
    }
  }

  lines.push(r.ok ? '\n✅ i18n 检查通过' : `\n❌ i18n 检查失败：${r.errors.length} 类问题`);
  console.log(lines.join('\n'));
  process.exit(r.ok ? 0 : 1);
}

// 仅当作为 CLI 直接执行时跑 main（被 vitest import 时不执行）。
// 用 pathToFileURL 比对而非字符串拼接：Windows 下盘符与反斜杠会让手工拼接失配。
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
