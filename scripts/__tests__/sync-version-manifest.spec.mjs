// @vitest-environment node
/**
 * sync-version-manifest.mjs 的单元测试。
 *
 * 覆盖三层：
 *  1. 解析层：parseLibrary / majorMinor / PRERELEASE_RE
 *  2. 策略层：四个 resolver 在给定 segments 下选出哪个 tag
 *  3. 契约层：cycleToId / buildExistingIdMap，以及「resolver 必须能重现
 *     manifest 里已有的 image_tag」这条金标准
 *
 * 第 3 层最关键：manifest 里所有条目都用短形式浮动 tag
 * （php:8.5-fpm / mysql:8.4 / redis:8.2-alpine / nginx:1.28-alpine），
 * 若 resolver 改成「完整版本号优先」，整份 manifest 会被判成 drift，
 * CI 的 --check 将永久 exit 1。这条测试就是防这个回归。
 */

import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import {
  SERVICES,
  parseLibrary,
  majorMinor,
  PRERELEASE_RE,
  phpStandardTag,
  mysqlStandardTag,
  redisStandardTag,
  nginxStandardTag,
  cycleToId,
  buildExistingIdMap,
  exitCodeFor,
} from '../sync-version-manifest.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const MANIFEST_PATH = join(
  __dirname,
  '..',
  '..',
  'src-tauri',
  'services',
  'version_manifest.json',
);
const manifest = JSON.parse(readFileSync(MANIFEST_PATH, 'utf8'));

// ────────────────────────────────────────────────────────────
// 1. 解析层
// ────────────────────────────────────────────────────────────

describe('parseLibrary', () => {
  it('按空行切段，提取 Tags 与 Directory', () => {
    const text = [
      'Tags: 8.5.1-fpm, 8.5-fpm, 8-fpm',
      'GitRepo: https://github.com/docker-library/php.git',
      'Directory: 8.5',
      '',
      'Tags: 8.4.15-fpm, 8.4-fpm',
      'Directory: 8.4',
    ].join('\n');

    const segs = parseLibrary(text);
    expect(segs).toHaveLength(2);
    expect(segs[0].tags).toEqual(['8.5.1-fpm', '8.5-fpm', '8-fpm']);
    expect(segs[0].directory).toBe('8.5');
    expect(segs[1].tags).toEqual(['8.4.15-fpm', '8.4-fpm']);
  });

  it('丢弃不含 Tags 的段（如纯注释块）', () => {
    const text = ['# some comment', 'GitRepo: x', '', 'Tags: 8.2-fpm', 'Directory: 8.2'].join('\n');
    const segs = parseLibrary(text);
    expect(segs).toHaveLength(1);
    expect(segs[0].tags).toEqual(['8.2-fpm']);
  });

  it('tag 列表去空格并过滤空项', () => {
    const segs = parseLibrary('Tags: 8.4-fpm ,  8.4 ,  \nDirectory: 8.4');
    expect(segs[0].tags).toEqual(['8.4-fpm', '8.4']);
  });

  it('缺少 Directory 时 directory 为空串而非 undefined', () => {
    const segs = parseLibrary('Tags: 8.4-fpm');
    expect(segs[0].directory).toBe('');
  });

  it('空输入返回空数组', () => {
    expect(parseLibrary('')).toEqual([]);
  });
});

describe('majorMinor', () => {
  it('从版本号 tag 提取 major.minor', () => {
    expect(majorMinor('8.5-fpm')).toBe('8.5');
    expect(majorMinor('8.5.1-fpm')).toBe('8.5');
    expect(majorMinor('1.28-alpine')).toBe('1.28');
  });

  it('非版本号别名返回 null', () => {
    expect(majorMinor('latest')).toBeNull();
    expect(majorMinor('alpine')).toBeNull();
    expect(majorMinor('mainline')).toBeNull();
    expect(majorMinor('fpm')).toBeNull();
  });
});

describe('PRERELEASE_RE', () => {
  it('识别连字符/点分隔的预发布标识', () => {
    expect(PRERELEASE_RE.test('8.6-rc')).toBe(true);
    expect(PRERELEASE_RE.test('8.6-rc1')).toBe(true);
    expect(PRERELEASE_RE.test('7.4.0-beta1')).toBe(true);
    expect(PRERELEASE_RE.test('8.0-preview')).toBe(true);
    expect(PRERELEASE_RE.test('8.5.0alpha2')).toBe(true);
  });

  // PHP 官方镜像的 RC tag 写作 8.5.0RC1 / 8.6.0beta1 —— 数字直接跟在关键字后，
  // 中间既没有 . 也没有 -。漏掉这种形式会让 RC 版本被当成稳定版写进 manifest。
  it('识别 PHP 风格、数字紧跟关键字的 RC/beta/alpha', () => {
    expect(PRERELEASE_RE.test('8.5.0RC1')).toBe(true);
    expect(PRERELEASE_RE.test('8.5.0RC1-fpm')).toBe(true);
    expect(PRERELEASE_RE.test('8.6.0beta1')).toBe(true);
    expect(PRERELEASE_RE.test('8.6.0alpha1')).toBe(true);
  });

  it('不误伤正式版本与常见别名', () => {
    expect(PRERELEASE_RE.test('8.5-fpm')).toBe(false);
    expect(PRERELEASE_RE.test('8.5.1-fpm')).toBe(false);
    expect(PRERELEASE_RE.test('8.4.7')).toBe(false);
    expect(PRERELEASE_RE.test('1.28-alpine')).toBe(false);
    expect(PRERELEASE_RE.test('8.2-alpine')).toBe(false);
    expect(PRERELEASE_RE.test('latest')).toBe(false);
    expect(PRERELEASE_RE.test('8.0.42')).toBe(false);
  });
});

// ────────────────────────────────────────────────────────────
// 2. 策略层
// ────────────────────────────────────────────────────────────

describe('phpStandardTag', () => {
  const segs = [{ tags: ['8.5.1-fpm', '8.5-fpm', '8-fpm', 'fpm'], directory: '8.5' }];

  it('短形式优先（与 manifest 的 php:8.5-fpm 一致）', () => {
    expect(phpStandardTag(segs, '8.5')).toBe('8.5-fpm');
  });

  it('没有短形式时回退到完整版本号', () => {
    expect(
      phpStandardTag([{ tags: ['8.5.1-fpm'], directory: '8.5' }], '8.5'),
    ).toBe('8.5.1-fpm');
  });

  it('跳过预发布 tag', () => {
    expect(
      phpStandardTag([{ tags: ['8.6.0RC1-fpm', '8.6-fpm'], directory: '8.6' }], '8.6'),
    ).toBe('8.6-fpm');
  });

  it('该 cycle 只有 RC 时不返回任何 tag（不能把 RC 当稳定版）', () => {
    expect(
      phpStandardTag([{ tags: ['8.6.0RC1-fpm'], directory: '8.6' }], '8.6'),
    ).toBeNull();
  });

  it('无匹配返回 null', () => {
    expect(phpStandardTag(segs, '7.4')).toBeNull();
  });
});

describe('mysqlStandardTag', () => {
  it('短形式优先（与 manifest 的 mysql:8.4 一致）', () => {
    const segs = [{ tags: ['8.4.7', '8.4', '8', 'latest'], directory: '8.4' }];
    expect(mysqlStandardTag(segs, '8.4')).toBe('8.4');
  });

  it('没有短形式时回退到最新完整版本号', () => {
    const segs = [{ tags: ['8.4.7', '8.4.6'], directory: '8.4' }];
    expect(mysqlStandardTag(segs, '8.4')).toBe('8.4.7');
  });

  it('跳过 innovation 段', () => {
    const segs = [
      { tags: ['9.5.0', '9.5', '9', 'innovation'], directory: 'innovation' },
      { tags: ['8.4.7', '8.4'], directory: '8.4' },
    ];
    expect(mysqlStandardTag(segs, '8.4')).toBe('8.4');
  });

  it('排除 oraclelinux / oracle / innodb / cluster 变种', () => {
    const segs = [
      { tags: ['8.4.7-oraclelinux9', '8.4-oracle', '8.4-innodb'], directory: '8.4' },
      { tags: ['8.4.7', '8.4'], directory: '8.4' },
    ];
    expect(mysqlStandardTag(segs, '8.4')).toBe('8.4');
  });

  it('无匹配返回 null', () => {
    expect(mysqlStandardTag([{ tags: ['8.4.7'], directory: '8.4' }], '5.7')).toBeNull();
  });
});

describe('redisStandardTag', () => {
  const segs = [
    { tags: ['8.2.3', '8.2', '8', 'latest'], directory: '8.2' },
    { tags: ['8.2.3-alpine', '8.2-alpine', '8-alpine', 'alpine'], directory: '8.2' },
  ];

  it('短形式 alpine 优先（与 manifest 的 redis:8.2-alpine 一致）', () => {
    expect(redisStandardTag(segs, '8.2')).toBe('8.2-alpine');
  });

  it('无短形式时用完整版 alpine', () => {
    const only = [{ tags: ['8.2.3', '8.2.3-alpine'], directory: '8.2' }];
    expect(redisStandardTag(only, '8.2')).toBe('8.2.3-alpine');
  });

  it('完全没有 alpine 时兜底到无后缀版本号', () => {
    const only = [{ tags: ['8.2.3', '8.2'], directory: '8.2' }];
    expect(redisStandardTag(only, '8.2')).toBe('8.2');
  });

  it('跳过预发布 tag', () => {
    const only = [{ tags: ['8.3-rc1-alpine', '8.2-alpine'], directory: '8.2' }];
    expect(redisStandardTag(only, '8.2')).toBe('8.2-alpine');
  });

  it('无匹配返回 null', () => {
    expect(redisStandardTag(segs, '6.0')).toBeNull();
  });
});

describe('nginxStandardTag', () => {
  const segs = [
    { tags: ['1.28.0', '1.28', 'stable', '1', 'latest'], directory: '1.28' },
    { tags: ['1.28.0-alpine', '1.28-alpine', 'stable-alpine'], directory: '1.28' },
  ];

  it('短形式 alpine 优先（与 manifest 的 nginx:1.28-alpine 一致）', () => {
    expect(nginxStandardTag(segs, '1.28')).toBe('1.28-alpine');
  });

  it('无 alpine 变体时用短形式版本号', () => {
    const only = [{ tags: ['1.28.0', '1.28', 'stable'], directory: '1.28' }];
    expect(nginxStandardTag(only, '1.28')).toBe('1.28');
  });

  it('只有完整版本号时兜底', () => {
    const only = [{ tags: ['1.28.0'], directory: '1.28' }];
    expect(nginxStandardTag(only, '1.28')).toBe('1.28.0');
  });

  it('无匹配返回 null', () => {
    expect(nginxStandardTag(segs, '1.20')).toBeNull();
  });
});

// ────────────────────────────────────────────────────────────
// 3. 契约层
// ────────────────────────────────────────────────────────────

describe('cycleToId', () => {
  it('nginx 用三位（1.28 → nginx128）', () => {
    expect(cycleToId('nginx', '1.28')).toBe('nginx128');
    expect(cycleToId('nginx', '1.9')).toBe('nginx109');
  });

  it('其他服务用两位', () => {
    expect(cycleToId('php', '8.5')).toBe('php85');
    expect(cycleToId('php', '5.6')).toBe('php56');
    expect(cycleToId('mysql', '8.4')).toBe('mysql84');
    expect(cycleToId('redis', '8.2')).toBe('redis82');
  });
});

describe('buildExistingIdMap', () => {
  it('从 image_tag 反推 svc:cycle → id', () => {
    const map = buildExistingIdMap(manifest);
    expect(map['php:8.5']).toBe('php85');
    expect(map['mysql:8.0']).toBe('mysql80');
    expect(map['redis:8.2']).toBe('redis82');
    expect(map['nginx:1.28']).toBe('nginx128');
  });

  it('manifest 里每个条目都能被反查到', () => {
    const map = buildExistingIdMap(manifest);
    for (const svc of SERVICES) {
      for (const id of Object.keys(manifest[svc] || {})) {
        const cycle = majorMinor(
          String(manifest[svc][id].image_tag).replace(/^[a-z]+:/, ''),
        );
        expect(map[`${svc}:${cycle}`], `${svc}.${id} 应可反查`).toBe(id);
      }
    }
  });

  it('缺 image_tag 的条目被跳过而不报错', () => {
    const map = buildExistingIdMap({ php: { php85: {} } });
    expect(map).toEqual({});
  });
});

describe('金标准：resolver 必须能重现 manifest 现有的 image_tag', () => {
  // 对每个条目构造一个「上游同时提供短形式与完整版本号」的 segment，
  // resolver 必须选中 manifest 里那个短形式。
  // 若哪天有人把策略改成「完整版本号优先」，整份 manifest 会被判 drift。
  const resolvers = {
    php: phpStandardTag,
    mysql: mysqlStandardTag,
    redis: redisStandardTag,
    nginx: nginxStandardTag,
  };

  for (const svc of SERVICES) {
    for (const [id, entry] of Object.entries(manifest[svc] || {})) {
      it(`${svc}.${id} → ${entry.image_tag}`, () => {
        const tag = String(entry.image_tag).replace(`${svc}:`, '');
        const cycle = majorMinor(tag);
        const segments = [
          // 完整版本号（模拟上游提供了具体 patch）
          { tags: [`${cycle}.99${tag.slice(cycle.length)}`, tag], directory: cycle },
          // 短形式（manifest 实际使用的）
          { tags: [tag, 'latest'], directory: cycle },
        ];
        expect(resolvers[svc](segments, cycle)).toBe(tag);
      });
    }
  }
});

describe('exitCodeFor', () => {
  const none = [];
  const oneFailure = [{ svc: 'php', cycle: '-', reason: '网络/解析失败: fetch failed' }];

  // 这条是全套测试里最要紧的一条：网络全挂时差异数必然为 0，
  // 若 --check 不阻断，CI 会把"什么都不知道"当成"完全同步"放绿灯。
  it('--check 在 fetch 失败时必须失败，不能因差异数为 0 而放行', () => {
    expect(
      exitCodeFor({ checkOnly: true, apply: false, hasDiff: false, fetchFailures: oneFailure }),
    ).toBe(1);
  });

  it('--check 在离线无缓存时同样失败', () => {
    const offline = [{ svc: 'php', cycle: '-', reason: '离线无缓存: ...' }];
    expect(
      exitCodeFor({ checkOnly: true, apply: false, hasDiff: false, fetchFailures: offline }),
    ).toBe(1);
  });

  it('--apply 在 fetch 失败时禁止写入并失败', () => {
    expect(
      exitCodeFor({ checkOnly: false, apply: true, hasDiff: true, fetchFailures: oneFailure }),
    ).toBe(1);
  });

  it('--check 有差异时失败', () => {
    expect(
      exitCodeFor({ checkOnly: true, apply: false, hasDiff: true, fetchFailures: none }),
    ).toBe(1);
  });

  it('--check 一切正常时通过', () => {
    expect(
      exitCodeFor({ checkOnly: true, apply: false, hasDiff: false, fetchFailures: none }),
    ).toBe(0);
  });

  it('默认报告模式即使有差异也不阻断（只打印报告）', () => {
    expect(
      exitCodeFor({ checkOnly: false, apply: false, hasDiff: true, fetchFailures: none }),
    ).toBe(0);
  });

  it('默认报告模式下 fetch 失败只降级、不阻断', () => {
    expect(
      exitCodeFor({ checkOnly: false, apply: false, hasDiff: false, fetchFailures: oneFailure }),
    ).toBe(0);
  });
});

describe('端到端链路：parseLibrary → resolver', () => {
  // 用贴近 docker-library/official-images 真实格式的文本跑完整解析链，
  // 覆盖单测里被人为拆开的 parseLibrary 与 resolver 之间的接口。
  const resolvers = {
    php: phpStandardTag,
    mysql: mysqlStandardTag,
    redis: redisStandardTag,
    nginx: nginxStandardTag,
  };

  const LIBRARY_FIXTURES = {
    php: [
      'Tags: 8.5.1-fpm, 8.5-fpm, 8-fpm, fpm',
      'GitRepo: https://github.com/docker-library/php.git',
      'Directory: 8.5',
      '',
      'Tags: 8.6.0RC1-fpm, 8.6.0RC1-fpm-alpine',
      'Directory: 8.6',
      '',
      'Tags: 5.6.40-fpm, 5.6-fpm',
      'Directory: 5.6',
    ].join('\n'),
    mysql: [
      'Tags: 9.5.0, 9.5, 9, innovation, latest',
      'Directory: innovation',
      '',
      'Tags: 8.4.7, 8.4, 8, oracle',
      'Directory: 8.4',
      '',
      'Tags: 8.4.7-oraclelinux9, 8.4-oraclelinux9',
      'Directory: 8.4',
    ].join('\n'),
    redis: [
      'Tags: 8.2.3, 8.2, 8, latest',
      'Directory: 8.2',
      '',
      'Tags: 8.2.3-alpine, 8.2-alpine, 8-alpine, alpine',
      'Directory: 8.2',
    ].join('\n'),
    nginx: [
      'Tags: 1.29.1, mainline, 1, 1.29, latest',
      'Directory: mainline',
      '',
      'Tags: 1.28.0, 1.28, stable, 1, latest',
      'Directory: stable',
      '',
      'Tags: 1.28.0-alpine, 1.28-alpine, stable-alpine, alpine',
      'Directory: stable',
    ].join('\n'),
  };

  it('PHP：从真实格式文本选出 php:8.5-fpm 对应的短形式', () => {
    expect(resolvers.php(parseLibrary(LIBRARY_FIXTURES.php), '8.5')).toBe('8.5-fpm');
  });

  it('PHP：RC-only 的 cycle 不产出稳定 tag', () => {
    expect(resolvers.php(parseLibrary(LIBRARY_FIXTURES.php), '8.6')).toBeNull();
  });

  it('PHP：老版本 5.6 仍能解析', () => {
    expect(resolvers.php(parseLibrary(LIBRARY_FIXTURES.php), '5.6')).toBe('5.6-fpm');
  });

  it('MySQL：跳过 innovation 与 oraclelinux 段，选 mysql:8.4', () => {
    expect(resolvers.mysql(parseLibrary(LIBRARY_FIXTURES.mysql), '8.4')).toBe('8.4');
  });

  it('Redis：跨段选出 redis:8.2-alpine', () => {
    expect(resolvers.redis(parseLibrary(LIBRARY_FIXTURES.redis), '8.2')).toBe('8.2-alpine');
  });

  it('Nginx：跨段选出 nginx:1.28-alpine', () => {
    expect(resolvers.nginx(parseLibrary(LIBRARY_FIXTURES.nginx), '1.28')).toBe('1.28-alpine');
  });

  it('Nginx：mainline cycle 同样遵循短形式优先', () => {
    expect(resolvers.nginx(parseLibrary(LIBRARY_FIXTURES.nginx), '1.29')).toBe('1.29');
  });
});
