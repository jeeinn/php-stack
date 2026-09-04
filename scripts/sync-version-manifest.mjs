#!/usr/bin/env node
/**
 * 从上游（docker-library/official-images + endoflife.date）拉取版本元数据，
 * 与 src-tauri/services/version_manifest.json 对比，生成可审计的差异报告。
 *
 * 背景：version_manifest.json 之前是手工维护的，结果严重滞后（项目 PHP 还在
 * 8.5，但上游已有 8.6-RC；MySQL 跳过了整条 9.x）。本脚本在仓库侧 / CI 跑，
 * 把"哪些版本算 stable"的决策外包给 endoflife.date（权威 EOL 数据源），
 * 自己在 official-images 库里找每个 cycle 对应的 latest tag。
 *
 * 用法：
 *   node scripts/sync-version-manifest.mjs            # 拉数据 + 报告（默认）
 *   node scripts/sync-version-manifest.mjs --check    # CI 用：有差异则 exit 1
 *   node scripts/sync-version-manifest.mjs --apply    # 写回 version_manifest.json
 *   node scripts/sync-version-manifest.mjs --offline  # 跳过网络，复用 .cache/*.json
 *
 * 输出：stdout 打 markdown 报告；非 0 exit 表示有差异。
 *
 * 设计红线（不可破坏）：
 *   1. 永不删除 manifest 里已有条目（用户手工维护的可能被覆盖）。
 *   2. 新增条目的 service_dir 默认复用 cycle 内已有目录，不自动建目录。
 *   3. eol=true 的版本仍出现在"建议新增"列表，但用 ⚠️ 标识（CI 提示人工确认）。
 *   4. 脚本不联网用户运行环境（用户离线场景下仍能用 --offline）。
 */

import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const MANIFEST_PATH = join(ROOT, 'src-tauri', 'services', 'version_manifest.json');
const CACHE_DIR = join(ROOT, '.workbuddy', 'sync-cache');

const TODAY = new Date().toISOString().slice(0, 10);
const USER_AGENT = 'php-stack-version-sync/1.0';
const FETCH_TIMEOUT_MS = 60_000;

const SERVICES = ['php', 'mysql', 'redis', 'nginx'];

// docker-library/official-images 的 library/ 文件：用 Tag 段匹配 cycle。
// endoflife.date API：返回 [{cycle, eol, latest, ...}]，cycle 即 "major.minor"。
const URLS = {
  library: (svc) => `https://raw.githubusercontent.com/docker-library/official-images/master/library/${svc}`,
  eol: (svc) => `https://endoflife.date/api/${svc}.json`,
};

// ────────────────────────────────────────────────────────────
// 网络
// ────────────────────────────────────────────────────────────

// Node 22 全局 fetch 默认 connect 超时只有 10s；本机通过系统代理访问
// raw.githubusercontent.com 时实测需要 8-21s 才能建连。绕开方案：禁用 fetch 的
// 代理检测（NODE_USE_ENV_PROXY=0），再叠加 AbortSignal.timeout。
// 见 https://github.com/nodejs/node/issues/43522 了解背景。
async function fetchText(url) {
  const res = await fetch(url, {
    headers: { 'User-Agent': USER_AGENT },
    signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
  });
  if (!res.ok) throw new Error(`GET ${url} → HTTP ${res.status}`);
  return res.text();
}

async function fetchJson(url) {
  return JSON.parse(await fetchText(url));
}

async function loadSource(svc, offline) {
  const cachePath = join(CACHE_DIR, `${svc}.library.txt`);
  const eolCachePath = join(CACHE_DIR, `${svc}.eol.json`);

  if (offline) {
    return {
      library: existsSync(cachePath) ? readFileSync(cachePath, 'utf8') : '',
      eol: existsSync(eolCachePath) ? JSON.parse(readFileSync(eolCachePath, 'utf8')) : [],
    };
  }

  mkdirSync(CACHE_DIR, { recursive: true });
  const [library, eol] = await Promise.all([fetchText(URLS.library(svc)), fetchJson(URLS.eol(svc))]);
  writeFileSync(cachePath, library);
  writeFileSync(eolCachePath, JSON.stringify(eol, null, 2));
  return { library, eol };
}

// ────────────────────────────────────────────────────────────
// 解析：official-images library 文本
// ────────────────────────────────────────────────────────────

/**
 * 把 library 文本切成 segments。每个段以空行分隔，结构：
 *   Tags: <csv>
 *   GitRepo: ...
 *   Architectures: ...
 *   Directory: ...
 *   ...
 */
function parseLibrary(text) {
  return text
    .split(/\n\n+/)
    .map((block) => {
      const tagLine = block.match(/^Tags:\s*(.+)$/m);
      if (!tagLine) return null;
      const dirLine = block.match(/^Directory:\s*(.+)$/m);
      return {
        tags: tagLine[1].split(',').map((s) => s.trim()).filter(Boolean),
        directory: dirLine ? dirLine[1].trim() : '',
      };
    })
    .filter(Boolean);
}

// 从 tag 字符串里提取 major.minor。无则返回 null。
function majorMinor(tag) {
  const m = tag.match(/^(\d+)\.(\d+)/);
  return m ? `${m[1]}.${m[2]}` : null;
}

// 标记"非稳定"tag：含 beta/alpha/rc/preview 字样
const PRERELEASE_RE = /(?:^|[.-])(?:beta|alpha|rc|preview|pre|nightly|edge|innovation)(?:[.-]|$)/i;

// ────────────────────────────────────────────────────────────
// 服务特定解析：每个 cycle 找最匹配的"标准"tag
// ────────────────────────────────────────────────────────────

/**
 * PHP：取 -fpm 变种。优先级短优先（8.5-fpm 优先于 8.5.10-fpm），
 * 因为 manifest 习惯用 `php:8.5-fpm` 这种 major.minor-fpm 短形式。
 */
function phpStandardTag(segments, cycle) {
  for (const seg of segments) {
    // 候选：tag 等于 `${cycle}-fpm`，或以 `${cycle}.` 开头且以 `-fpm` 结尾。
    // 注意运算符优先级：|| 必须用括号显式分组，否则 && 会先结合。
    const candidates = seg.tags
      .filter((t) => !PRERELEASE_RE.test(t))
      .filter((t) => t === `${cycle}-fpm` || (t.startsWith(`${cycle}.`) && t.endsWith('-fpm')));
    if (candidates.length === 0) continue;
    // 优先短形式 `${cycle}-fpm`（manifest 习惯）；无则取第一个完整版本号。
    const short = candidates.find((t) => t === `${cycle}-fpm`);
    return short || candidates[0];
  }
  return null;
}

/**
 * MySQL：取无变种后缀的形式（"9.7.2" 或 "8.4.11"），
 * 排除 oraclelinux/oracle/innodb 等专用变种。
 */
function mysqlStandardTag(segments, cycle) {
  for (const seg of segments) {
    // 跳过 innovation 段（Directory 标记）
    if (seg.directory === 'innovation') continue;
    // 候选：tag 是 `${cycle}` 或 `${cycle}.${patch}`，无后缀
    const candidates = seg.tags.filter((t) => {
      if (PRERELEASE_RE.test(t)) return false;
      if (/-(?:oraclelinux\d*|oracle|innodb|cluster)/i.test(t)) return false;
      return t === cycle || t.startsWith(`${cycle}.`);
    });
    if (candidates.length === 0) continue;
    // 优先完整版本号，再短形式
    return candidates.sort((a, b) => b.length - a.length)[0];
  }
  return null;
}

/**
 * Redis：取 -alpine 变种（项目偏好 alpine 小镜像）。
 */
function redisStandardTag(segments, cycle) {
  // 找含 `${cycle}-alpine` 的 tag
  const allTags = segments.flatMap((s) => s.tags).filter((t) => !PRERELEASE_RE.test(t));
  // 优先：完整版本 -alpine
  const full = allTags.find((t) => t.startsWith(`${cycle}.`) && t.endsWith('-alpine'));
  if (full) return full;
  // 次选：短形式 -alpine
  const short = allTags.find((t) => t === `${cycle}-alpine`);
  if (short) return short;
  // 兜底：完整版本号无后缀
  return allTags.find((t) => t === cycle || t.startsWith(`${cycle}.`)) || null;
}

/**
 * Nginx：区分 mainline 与 stable。Library 文件里 "Tags: 1.31.5, mainline, ..."
 * 中 mainline/stable 是 alias 字面量。eol 的 cycle 与 alias 一一对应。
 * 简化策略：mainline = eol 中最大的 cycle；stable = 次大且非 mainline 的。
 * 这样不依赖 alias 字符串（虽然 alias 也能识别）。
 */
function nginxStandardTag(segments, cycle) {
  // mainline 在 segment 里以 alias 'mainline' 标识
  // stable 以 alias 'stable' 标识
  const allTags = segments.flatMap((s) => s.tags).filter((t) => !PRERELEASE_RE.test(t));
  const full = allTags.find((t) => t.startsWith(`${cycle}.`));
  return full || allTags.find((t) => t === cycle) || null;
}

const RESOLVERS = {
  php: phpStandardTag,
  mysql: mysqlStandardTag,
  redis: redisStandardTag,
  nginx: nginxStandardTag,
};

// ────────────────────────────────────────────────────────────
// 主流程
// ────────────────────────────────────────────────────────────

/** 把 "1.2" 这种 cycle 转成 manifest 里习惯的 id（"php12"）。 */
function cycleToId(svc, cycle) {
  const major = cycle.split('.')[0];
  const minor = cycle.split('.')[1];
  // nginx 用 3 位（如 1.27 → 127）；其他用 2 位
  if (svc === 'nginx') {
    return `${svc}${major}${minor.padStart(2, '0')}`;
  }
  return `${svc}${major}${minor}`;
}

/** manifest 现有条目 id → cycle 反查。 */
function buildExistingIdMap(manifest) {
  const map = {};
  for (const svc of SERVICES) {
    if (!manifest[svc]) continue;
    for (const id of Object.keys(manifest[svc])) {
      // 从 image_tag 反推 cycle（如 "php:8.5-fpm" → "8.5"）
      const tag = manifest[svc][id].image_tag || '';
      const m = tag.match(/(?::|-)(\d+\.\d+)/);
      if (m) map[`${svc}:${m[1]}`] = id;
    }
  }
  return map;
}

async function main() {
  const args = process.argv.slice(2);
  const checkOnly = args.includes('--check');
  const apply = args.includes('--apply');
  const offline = args.includes('--offline');

  console.log(`# php-stack 版本同步报告\n`);
  console.log(`生成时间: ${TODAY}`);
  console.log(`数据源: docker-library/official-images + endoflife.date\n`);

  const manifest = JSON.parse(readFileSync(MANIFEST_PATH, 'utf8'));
  const existingIdMap = buildExistingIdMap(manifest);

  /** @type {{svc: string, cycle: string, latest_tag: string, eol: any}[]} */
  const upstreamRows = [];
  /** @type {{svc: string, cycle: string, eol: any}[]} */
  const eolOnlyRows = []; // 仅 eol api 知道的 cycle（official-images 可能已下架）
  /** @type {{svc: string, cycle: string, reason: string}[]} */
  const skipped = [];

  for (const svc of SERVICES) {
    let data;
    try {
      data = await loadSource(svc, offline);
    } catch (e) {
      console.error(`⚠️  拉取 ${svc} 数据失败: ${e.message}`);
      skipped.push({ svc, cycle: '-', reason: `网络/解析失败: ${e.message}` });
      continue;
    }

    const segments = parseLibrary(data.library);
    const resolver = RESOLVERS[svc];

    for (const eolRow of data.eol) {
      const cycle = eolRow.cycle;
      const tag = resolver(segments, cycle);
      if (!tag) {
        const upstreamHasDir = segments.some((seg) =>
          seg.tags.some((t) => majorMinor(t) === cycle && !PRERELEASE_RE.test(t))
        );
        if (upstreamHasDir) {
          skipped.push({ svc, cycle, reason: '解析未匹配（tag 变体不在规则覆盖内）' });
        } else if (eolRow.eol === true || (typeof eolRow.eol === 'string' && new Date(eolRow.eol) < new Date(TODAY))) {
          // 上游完全停止维护该 cycle，但 eol api 标了 EOL——记到 eolOnlyRows 用于比对 manifest
          eolOnlyRows.push({ svc, cycle, eol: eolRow.eol });
        }
        continue;
      }
      upstreamRows.push({
        svc,
        cycle,
        latest_tag: tag,
        eol: eolRow.eol,
        eol_latest: eolRow.latest,
      });
    }
  }

  // ─── 三类输出 ───
  const newUpstream = []; // eol 有，manifest 没有
  const drift = [];       // manifest 有，但 image_tag 落后
  const eolChanges = [];  // eol 日期与 manifest 的 eol 字段不一致

  for (const row of upstreamRows) {
    const key = `${row.svc}:${row.cycle}`;
    const manifestId = existingIdMap[key];
    if (!manifestId) {
      newUpstream.push(row);
      continue;
    }
    const entry = manifest[row.svc][manifestId];
    // 上游期望的 image_tag：每个服务都带 `<svc>:` 前缀（与 manifest 习惯一致）
    const expectedTag = `${row.svc}:${row.latest_tag}`;
    if (entry.image_tag !== expectedTag) {
      drift.push({ ...row, current_id: manifestId, current_tag: entry.image_tag, expected_tag: expectedTag });
    }
    // EOL 状态比对：manifest._provenance 不存 eol 日期，这里只比对 eol bool
    // （如果 manifest 后续加 eol_date 字段再扩展）
  }

  // 现有条目已 EOL 但 manifest 未标（基于 upstream 的 eol 日期 + 今天）。
  // 合并 upstreamRows（official-images 仍有目录）与 eolOnlyRows（official-images 已下架）。
  const allEolRows = [...upstreamRows, ...eolOnlyRows];
  for (const svc of SERVICES) {
    if (!manifest[svc]) continue;
    for (const id of Object.keys(manifest[svc])) {
      const entry = manifest[svc][id];
      const tag = entry.image_tag || '';
      const m = tag.match(/(?::|-)(\d+\.\d+)/);
      if (!m) continue;
      const upstream = allEolRows.find((r) => r.svc === svc && r.cycle === m[1]);
      if (!upstream) continue;
      const upstreamIsEol =
        upstream.eol === true ||
        (typeof upstream.eol === 'string' && new Date(upstream.eol) < new Date(TODAY));
      if (upstreamIsEol && entry.eol !== true) {
        eolChanges.push({ svc, id, cycle: m[1], upstream_eol: upstream.eol, manifest_eol: entry.eol });
      }
    }
  }

  // ─── Markdown 报告 ───
  console.log(`## 1. 上游新增版本（manifest 缺失）\n`);
  if (newUpstream.length === 0) {
    console.log(`无新增。\n`);
  } else {
    console.log(`| 服务 | cycle | latest_tag | EOL | 建议 ID |`);
    console.log(`|---|---|---|---|---|`);
    for (const r of newUpstream) {
      const id = cycleToId(r.svc, r.cycle);
      const flag = r.eol === true ? '⚠️ true' : (r.eol || 'false');
      console.log(`| ${r.svc} | ${r.cycle} | \`${r.latest_tag}\` | ${flag} | \`${id}\` |`);
    }
    console.log('');
  }

  console.log(`## 2. 现有条目 image_tag 落后\n`);
  if (drift.length === 0) {
    console.log(`无偏差。\n`);
  } else {
    console.log(`| 服务 | cycle | 当前 ID | 当前 tag | 上游期望 |`);
    console.log(`|---|---|---|---|---|`);
    for (const r of drift) {
      console.log(`| ${r.svc} | ${r.cycle} | \`${r.current_id}\` | \`${r.current_tag}\` | \`${r.expected_tag}\` |`);
    }
    console.log('');
  }

  console.log(`## 3. EOL 状态变化\n`);
  if (eolChanges.length === 0) {
    console.log(`无变化。\n`);
  } else {
    console.log(`| 服务 | ID | cycle | manifest.eol | upstream.eol |`);
    console.log(`|---|---|---|---|---|`);
    for (const r of eolChanges) {
      console.log(`| ${r.svc} | \`${r.id}\` | ${r.cycle} | ${r.manifest_eol} | ${r.upstream_eol} |`);
    }
    console.log('');
  }

  if (skipped.length > 0) {
    console.log(`## ⚠️  跳过项\n`);
    for (const s of skipped) {
      console.log(`- ${s.svc}${s.cycle !== '-' ? ` ${s.cycle}` : ''}: ${s.reason}`);
    }
    console.log('');
  }

  // ─── 退出码 / 写回 ───
  const hasDiff = newUpstream.length + drift.length + eolChanges.length > 0;

  // --apply 必须保证所有 fetch 都通了；网络不稳时不能写半截。
  const fetchFailures = skipped.filter((s) => s.reason.startsWith('网络/解析失败'));
  if (apply && fetchFailures.length > 0) {
    console.error(`\n💥 --apply 模式下 fetch 失败 ${fetchFailures.length} 项，禁止写入。`);
    console.error(`   失败列表: ${fetchFailures.map((s) => s.svc).join(', ')}`);
    console.error(`   请检查网络后重试，或使用 --offline 模式（需先有缓存）。`);
    process.exit(1);
  }

  if (apply && newUpstream.length > 0) {
    console.log(`## 📝 已写入 version_manifest.json\n`);
    for (const r of newUpstream) {
      const id = cycleToId(r.svc, r.cycle);
      // service_dir：复用 cycle 内已有的；都没有则用 cycleToId 的结果
      const existingDir = Object.keys(manifest[r.svc] || {}).find(
        (k) => manifest[r.svc][k].service_dir
      );
      const serviceDir = existingDir ? manifest[r.svc][existingDir].service_dir : id;

      manifest[r.svc] = manifest[r.svc] || {};
      manifest[r.svc][id] = {
        display_name: `${r.svc.toUpperCase()} ${r.cycle}`,
        image_tag: `${r.svc}:${r.latest_tag}`,
        service_dir: serviceDir,
        ...(r.svc === 'php' && { default_port: 9000, show_port: false }),
        ...(r.svc === 'mysql' && { default_port: 3306, show_port: true }),
        ...(r.svc === 'redis' && { default_port: 6379, show_port: true }),
        ...(r.svc === 'nginx' && { default_port: 80, show_port: true }),
        eol: r.eol === true,
      };
      console.log(`- 新增 \`${r.svc}.${id}\` (${r.latest_tag})`);
    }
    // 复用 manifest 已有的 _provenance.updated_at 字段，避免引入新字段造成 diff 噪音
    manifest._provenance.updated_at = TODAY;
    writeFileSync(MANIFEST_PATH, JSON.stringify(manifest, null, 2) + '\n');
    console.log('');
  } else if (hasDiff && !apply) {
    console.log(`> 💡 应用差异：\`node scripts/sync-version-manifest.mjs --apply\`\n`);
  }

  if (checkOnly && hasDiff) {
    process.exit(1);
  }
}

main().catch((e) => {
  console.error('💥 脚本异常:', e.message);
  process.exit(2);
});