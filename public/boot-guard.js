/**
 * 启动期错误兜底：把「白屏」变成可读的报错。
 *
 * 生产包未启用 devtools（Cargo.toml 里 tauri features 为空），安装后一旦前端
 * 没跑起来，用户看到的是纯白窗口，而日志里只有一行 `PHP-Stack started` ——
 * 无从下手。本脚本在业务代码之前执行，负责三件事：
 *
 *   1. 捕获 JS 异常、未处理的 Promise rejection、console.error
 *   2. 捕获**资源加载失败**（script/link 加载失败不抛异常，只在 target 元素上
 *      触发 error 事件，且必须走捕获阶段）—— 白屏最常见的原因正是 assets 404
 *   3. 挂载迟迟没发生时，直接把错误渲染到 #app，并附日志文件路径
 *
 * 两个约束：
 * - 必须是普通脚本（非 module）：module 默认 defer，晚于业务代码；且业务代码
 *   加载失败时 module 图整体不可用。
 * - 必须是外部文件：tauri.conf.json 的 CSP 未声明 script-src，回退 default-src
 *   'self'，内联脚本会被拦。public/ 下的文件原样拷进 dist，同源加载不受影响。
 * - 不能依赖 @tauri-apps/api：若它是加载失败的那一个，import 也会失败。改用
 *   Tauri 注入的内部句柄 window.__TAURI_INTERNALS__.invoke。
 */
(function (global) {
  'use strict';

  /** 最多上报到日志的条数（超出只留在内存里给面板显示，避免刷屏）。 */
  var MAX_REPORTS = 5;
  /** 单字段上报上限，与 Rust 侧 FRONTEND_ERROR_FIELD_LIMIT 对齐。 */
  var MAX_FIELD = 2000;
  /** 等挂载的宽限时间：Vue 正常挂载远快于此，超时即认为启动失败。 */
  var FALLBACK_DELAY_MS = 600;

  var entries = [];
  var reported = 0;
  var mounted = false;
  var fallbackRendered = false;

  function truncate(value, limit) {
    if (value === null || value === undefined) return '';
    var v = String(value).trim();
    if (v.length <= limit) return v;
    return v.slice(0, limit) + '...(truncated)';
  }

  /**
   * 归一化成与 Rust 侧 FrontendErrorReport 一致的形状。
   * @returns {{ message: string, source: string, stack: string, location: string }}
   */
  function normalize(raw) {
    var input = raw || {};
    var error = input.error;
    var message = input.message;
    if (!message && error && error.message) message = error.message;
    if (!message && input.reason !== undefined && input.reason !== null) {
      message = String(input.reason);
    }

    var location = input.location;
    if (!location && (input.lineno || input.colno)) {
      location = (input.filename || '') + ':' + (input.lineno || 0) + ':' + (input.colno || 0);
    }

    return {
      message: truncate(message, MAX_FIELD) || '(empty error message)',
      source: truncate(input.source, 120) || 'unknown',
      stack: truncate(input.stack || (error && error.stack), MAX_FIELD),
      location: truncate(location, 500),
    };
  }

  /** 去重键：同一错误在多个钩子里（onerror + console.error）会重复出现。 */
  function dedupeKey(entry) {
    return entry.source + '|' + entry.message + '|' + entry.location;
  }

  function invokeTauri(payload) {
    try {
      var internals = global.__TAURI_INTERNALS__;
      if (!internals || typeof internals.invoke !== 'function') return false;
      var result = internals.invoke('log_frontend_error', { report: payload });
      // invoke 返回 Promise；白屏场景下它本身也可能失败，一律静默
      if (result && typeof result.catch === 'function') {
        result.catch(function () {});
      }
      return true;
    } catch (e) {
      return false;
    }
  }

  function report(raw) {
    var entry = normalize(raw);
    for (var i = 0; i < entries.length; i++) {
      if (dedupeKey(entries[i]) === dedupeKey(entry)) return entries[i];
    }
    entries.push(entry);

    if (reported < MAX_REPORTS) {
      reported++;
      invokeTauri(entry);
    }
    scheduleFallback();
    return entry;
  }

  function isMounted() {
    if (mounted) return true;
    var host = global.document && global.document.getElementById('app');
    return !!(host && host.childElementCount > 0);
  }

  function scheduleFallback() {
    if (fallbackRendered) return;
    var doc = global.document;
    if (!doc) return;

    var run = function () {
      if (fallbackRendered || isMounted()) return;
      fallbackRendered = true;
      renderFallback();
    };

    if (doc.readyState === 'complete' || doc.readyState === 'interactive') {
      global.setTimeout(run, FALLBACK_DELAY_MS);
    } else {
      doc.addEventListener('DOMContentLoaded', function () {
        global.setTimeout(run, FALLBACK_DELAY_MS);
      });
    }
  }

  /** 用 DOM API 构建（不拼 innerHTML），错误信息里可能含 HTML。 */
  function renderFallback() {
    var doc = global.document;
    var host = doc && doc.getElementById('app');
    if (!host) return false;

    host.textContent = '';

    var box = doc.createElement('div');
    box.setAttribute(
      'style',
      'padding:24px;font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;' +
        'color:#b91c1c;font-size:13px;line-height:1.7;'
    );

    var title = doc.createElement('div');
    title.setAttribute('style', 'font-size:15px;font-weight:600;margin-bottom:12px;');
    title.textContent = 'Application failed to start';
    box.appendChild(title);

    var hint = doc.createElement('div');
    hint.setAttribute('style', 'color:#475569;margin-bottom:16px;');
    hint.textContent =
      'The interface did not load. Details below are also written to the log file:';
    box.appendChild(hint);

    // 路径按平台静态列出（bundle 目标是 all）。不额外 invoke 查询：白屏时
    // 前端 bundle 可能根本没跑起来，兜底自身依赖越少越可靠。
    var logPath = doc.createElement('div');
    logPath.setAttribute(
      'style',
      'color:#475569;margin-bottom:16px;white-space:pre-wrap;word-break:break-all;'
    );
    logPath.textContent =
      'Log file (Windows): %APPDATA%\\com.php-stack.dev\\php-stack.log\n' +
      'Log file (macOS):   ~/Library/Application Support/com.php-stack.dev/php-stack.log';
    box.appendChild(logPath);

    var list = doc.createElement('ul');
    list.setAttribute('style', 'margin:0;padding-left:18px;');
    var shown = entries.length === 0 ? [{ message: '(no error captured — the script bundle may never have executed)', source: 'boot-guard', stack: '', location: '' }] : entries;
    for (var i = 0; i < shown.length; i++) {
      var item = doc.createElement('li');
      item.setAttribute('style', 'margin-bottom:10px;white-space:pre-wrap;word-break:break-word;');
      item.textContent = '[' + shown[i].source + '] ' + shown[i].message;
      if (shown[i].location) {
        item.textContent += '\n  at ' + shown[i].location;
      }
      if (shown[i].stack) {
        item.textContent += '\n' + shown[i].stack;
      }
      list.appendChild(item);
    }
    box.appendChild(list);

    host.appendChild(box);
    return true;
  }

  // --- 钩子安装 -------------------------------------------------------------

  global.addEventListener(
    'error',
    function (event) {
      // 资源加载失败：事件在元素上触发，不会冒泡，必须捕获阶段
      var target = event.target;
      if (target && target !== global && target.tagName) {
        var tag = target.tagName.toUpperCase();
        if (tag === 'SCRIPT' || tag === 'LINK' || tag === 'IMG') {
          report({
            message: 'Failed to load ' + tag.toLowerCase() + ': ' + (target.src || target.href || '(unknown url)'),
            source: 'resource',
            location: target.src || target.href || '',
          });
          return;
        }
      }
      report({
        message: event.message,
        source: 'window.onerror',
        error: event.error,
        filename: event.filename,
        lineno: event.lineno,
        colno: event.colno,
      });
    },
    true
  );

  global.addEventListener('unhandledrejection', function (event) {
    report({
      message: 'Unhandled promise rejection',
      reason: event.reason,
      source: 'unhandledrejection',
      stack: event.reason && event.reason.stack,
    });
  });

  // 生产包没有 devtools，console.error 是唯一能看到 Vue 警告/错误的通道
  var originalConsoleError = global.console && global.console.error;
  if (typeof originalConsoleError === 'function') {
    global.console.error = function () {
      try {
        var parts = [];
        for (var i = 0; i < arguments.length; i++) {
          var a = arguments[i];
          parts.push(a && a.stack ? a.stack : String(a));
        }
        report({ message: parts.join(' '), source: 'console.error' });
      } catch (e) {
        /* 兜底自身不能再抛 */
      }
      return originalConsoleError.apply(global.console, arguments);
    };
  }

  // 兜底：即便没有任何错误事件，只要页面迟迟没挂载出来也给出提示
  scheduleFallback();

  global.__phpStackBootGuard = {
    report: report,
    markMounted: function () {
      mounted = true;
    },
    entries: function () {
      return entries.slice();
    },
    /** 测试用：内部函数无副作用暴露 */
    __test: {
      normalize: normalize,
      truncate: truncate,
      isMounted: isMounted,
      renderFallback: renderFallback,
    },
  };
})(typeof window !== 'undefined' ? window : this);
