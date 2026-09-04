#!/usr/bin/env node
/**
 * 同步各 PHP 版本目录下的 Dockerfile。
 *
 * 背景：services/phpXX/Dockerfile 共 8 份，内容仅两行不同（版本注释与
 * ARG PHP_BASE_IMAGE 的默认值），其余完全一致。由于 config_generator.rs
 * 的 resolve_template_dir() 要求每个 service_dir 都真实存在 Dockerfile
 * （key_file 校验），无法物理合并成一份文件，只能靠脚本保证同步。
 *
 * 用法：
 *   node scripts/sync-php-dockerfile.mjs           # 同步写入
 *   node scripts/sync-php-dockerfile.mjs --check   # 只检查，不同步（CI 用）
 *
 * 约定：php85 是模板源（canonical source）。改 Dockerfile 时改 php85 那份，
 * 然后跑本脚本同步其余版本。新增版本时在 MAP 里加一行即可。
 */

import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const SERVICES_DIR = join(__dirname, '..', 'src-tauri', 'services');

/** service_dir → PHP 版本号（用于 image_tag 与 env 前缀注释） */
const MAP = {
  php56: '5.6',
  php74: '7.4',
  php80: '8.0',
  php81: '8.1',
  php82: '8.2',
  php83: '8.3',
  php84: '8.4',
  php85: '8.5',
};

/** 模板源：改 Dockerfile 请改这一份 */
const TEMPLATE_DIR = 'php85';
const TEMPLATE_VERSION = MAP[TEMPLATE_DIR];

const checkOnly = process.argv.includes('--check');

/**
 * 把模板内容里的版本相关片段替换为指定版本。
 * 只动两处：注释中的 PHP85_VERSION，以及 ARG 的默认值 php:8.5-fpm。
 */
function renderFor(template, dir, version) {
  const prefix = dir.toUpperCase();
  return template
    .replace(
      // 注意：dir.toUpperCase() 已含 "PHP" 前缀（如 php84 → PHP84），不可再拼 PHP
      new RegExp(`${TEMPLATE_DIR.toUpperCase()}_VERSION`, 'g'),
      `${prefix}_VERSION`,
    )
    .replace(
      new RegExp(`ARG PHP_BASE_IMAGE=php:${escapeVer(TEMPLATE_VERSION)}-fpm`, 'g'),
      `ARG PHP_BASE_IMAGE=php:${version}-fpm`,
    )
    .replace(
      new RegExp(`\\(e\\.g\\., php:${escapeVer(TEMPLATE_VERSION)}-fpm-alpine\\)`, 'g'),
      `(e.g., php:${version}-fpm-alpine)`,
    );
}

function escapeVer(v) {
  return v.replace(/\./g, '\\.');
}

function main() {
  const templatePath = join(SERVICES_DIR, TEMPLATE_DIR, 'Dockerfile');
  if (!existsSync(templatePath)) {
    console.error(`模板不存在: ${templatePath}`);
    process.exit(1);
  }
  const template = readFileSync(templatePath, 'utf8');

  let changed = 0;
  let dirty = 0;

  for (const [dir, version] of Object.entries(MAP)) {
    const target = join(SERVICES_DIR, dir, 'Dockerfile');
    const expected = renderFor(template, dir, version);

    if (!existsSync(target)) {
      if (checkOnly) {
        console.error(`[缺失] services/${dir}/Dockerfile`);
        dirty++;
      } else {
        writeFileSync(target, expected, 'utf8');
        console.log(`[新建] services/${dir}/Dockerfile  (php:${version}-fpm)`);
        changed++;
      }
      continue;
    }

    const current = readFileSync(target, 'utf8');
    if (current === expected) continue;

    if (checkOnly) {
      console.error(`[不同步] services/${dir}/Dockerfile`);
      dirty++;
    } else {
      writeFileSync(target, expected, 'utf8');
      console.log(`[已同步] services/${dir}/Dockerfile  (php:${version}-fpm)`);
      changed++;
    }
  }

  if (checkOnly) {
    if (dirty > 0) {
      console.error(
        `\n${dirty} 份不同步。请运行 node scripts/sync-php-dockerfile.mjs 后提交。`,
      );
      process.exit(1);
    }
    console.log(`全部 ${Object.keys(MAP).length} 份 Dockerfile 已同步。`);
  } else {
    console.log(
      changed === 0
        ? `全部 ${Object.keys(MAP).length} 份 Dockerfile 已同步，无需改动。`
        : `已同步 ${changed} 份 Dockerfile。`,
    );
  }
}

main();
