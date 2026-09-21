// @vitest-environment node
/**
 * sync-php-dockerfile.mjs 的单元测试。
 *
 * 分两层：
 *  1. 纯函数层：escapeVer / renderFor 的行为契约
 *  2. 金标准层：把磁盘上真实的 8 份 Dockerfile 与「从 php85 模板渲染出的期望内容」
 *     逐一比对 —— 等价于跑一遍 --check，把 C5b 产物的同步状态锁进测试。
 *
 * 第 2 层是这里最有价值的部分：它把「改了模板忘了同步」从
 * 「CI 跑 --check 才报错」提前到「单元测试立刻红」。
 */

import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import {
  MAP,
  TEMPLATE_DIR,
  TEMPLATE_VERSION,
  escapeVer,
  renderFor,
} from '../sync-php-dockerfile.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const SERVICES_DIR = join(__dirname, '..', '..', 'src-tauri', 'services');

const readTemplate = () =>
  readFileSync(join(SERVICES_DIR, TEMPLATE_DIR, 'Dockerfile'), 'utf8');

describe('escapeVer', () => {
  it('把版本号里的点转义成正则字面量点', () => {
    expect(escapeVer('8.5')).toBe('8\\.5');
    expect(escapeVer('5.6')).toBe('5\\.6');
  });

  it('转义后能被 RegExp 安全使用（不会把 . 当通配符）', () => {
    // 未转义的话 '8.5' 里的 . 会匹配任意字符，"8x5" 也会命中
    expect(new RegExp(escapeVer('8.5')).test('8x5')).toBe(false);
    expect(new RegExp(escapeVer('8.5')).test('8.5')).toBe(true);
  });
});

describe('renderFor — 三处版本相关替换', () => {
  const template = readTemplate();

  it('替换 env 前缀注释里的 PHP85_VERSION', () => {
    const out = renderFor(template, 'php56', '5.6');
    expect(out).toContain('PHP56_VERSION');
    expect(out).not.toContain('PHP85_VERSION');
  });

  it('替换 ARG PHP_BASE_IMAGE 的默认值', () => {
    const out = renderFor(template, 'php56', '5.6');
    expect(out).toContain('ARG PHP_BASE_IMAGE=php:5.6-fpm');
    expect(out).not.toContain('ARG PHP_BASE_IMAGE=php:8.5-fpm');
  });

  it('替换 e.g. 注释里的示例版本号', () => {
    const out = renderFor(template, 'php74', '7.4');
    expect(out).toContain('(e.g., php:7.4-fpm-alpine)');
    expect(out).not.toContain('(e.g., php:8.5-fpm-alpine)');
  });

  it('对模板源自身是幂等的', () => {
    expect(renderFor(template, TEMPLATE_DIR, TEMPLATE_VERSION)).toBe(template);
  });

  it('不误伤与版本无关的内容（ARG 名、安装脚本、WORKDIR）', () => {
    const out = renderFor(template, 'php56', '5.6');
    expect(out).toContain('ARG PHP_EXTENSIONS');
    expect(out).toContain('install-php-extensions');
    expect(out).toContain('WORKDIR /www');
    expect(out).toContain('ARG DEBIAN_MIRROR_DOMAIN');
  });

  it('保留模板原有行数（只做同构替换，不增删行）', () => {
    const out = renderFor(template, 'php83', '8.3');
    expect(out.split('\n').length).toBe(template.split('\n').length);
  });

  it('MAP 每个目录都能渲染出带自身前缀的结果', () => {
    for (const [dir, version] of Object.entries(MAP)) {
      const out = renderFor(template, dir, version);
      expect(out, `${dir} 应含 ${dir.toUpperCase()}_VERSION`).toContain(
        `${dir.toUpperCase()}_VERSION`,
      );
      expect(out, `${dir} 应含 ARG PHP_BASE_IMAGE=php:${version}-fpm`).toContain(
        `ARG PHP_BASE_IMAGE=php:${version}-fpm`,
      );
    }
  });
});

describe('金标准：磁盘上的 8 份 Dockerfile 与模板渲染结果一致', () => {
  // 这条等价于 `npm run check:dockerfile`，但被单元测试接管：
  // 谁改了 php85/Dockerfile 却忘了跑同步，这里会立刻红。
  const template = readTemplate();

  for (const [dir, version] of Object.entries(MAP)) {
    it(`services/${dir}/Dockerfile 已同步`, () => {
      const actual = readFileSync(join(SERVICES_DIR, dir, 'Dockerfile'), 'utf8');
      const expected = renderFor(template, dir, version);
      expect(actual).toBe(expected);
    });
  }
});
