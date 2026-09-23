/**
 * boot-guard.js 是普通脚本（非 module），无法 import，用 vm 在自建 context
 * 里执行后断言它挂到 window 上的行为。
 *
 * 关键回归点：
 * - 资源加载失败（script/link 404）**不会**抛 JS 异常，只在元素上触发 error
 *   事件 —— 这是白屏最常见的原因，一旦漏掉这个分支，兜底形同虚设
 * - 上报载荷形状必须与 Rust 侧 FrontendErrorReport 对齐
 */
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import vm from 'node:vm'
import { describe, expect, it } from 'vitest'

// 被测脚本放在 public/ 而非 src/：它需要被 Vite 原样拷进 dist 并同源加载，
// 而 src/ 下的文件要走打包。测试则不能放 public/ —— 会一起被拷进产物。
const SCRIPT = readFileSync(
  join(__dirname, '..', '..', 'public', 'boot-guard.js'),
  'utf8'
)

interface Guard {
  report: (raw: Record<string, unknown>) => {
    message: string
    source: string
    stack: string
    location: string
  }
  markMounted: () => void
  entries: () => Array<{ message: string; source: string; location: string }>
  __test: {
    normalize: (raw: Record<string, unknown>) => Record<string, string>
    truncate: (v: unknown, limit: number) => string
    isMounted: () => boolean
    renderFallback: () => boolean
  }
}

interface Harness {
  guard: Guard
  host: { childElementCount: number; textContent: string; appended: unknown[] }
  timers: Array<() => void>
  invokes: Array<{ cmd: string; args: unknown }>
  handlers: Record<string, (event: unknown) => void>
  flushTimers: () => void
}

function createElement() {
  return {
    children: [] as unknown[],
    attrs: {} as Record<string, string>,
    textContent: '',
    setAttribute(k: string, v: string) {
      this.attrs[k] = v
    },
    appendChild(child: unknown) {
      this.children.push(child)
    },
  }
}

function setup(): Harness {
  const host = {
    childElementCount: 0,
    textContent: '' as string,
    appended: [] as unknown[],
    setAttribute() {},
    appendChild(child: unknown) {
      this.appended.push(child)
      this.childElementCount = this.appended.length
    },
  }

  const timers: Array<() => void> = []
  const invokes: Array<{ cmd: string; args: unknown }> = []
  const handlers: Record<string, (event: unknown) => void> = {}

  const document = {
    readyState: 'complete',
    getElementById: (id: string) => (id === 'app' ? host : null),
    createElement,
    addEventListener() {},
  }

  const sandbox: Record<string, unknown> = {
    document,
    console: { error: () => {} },
    setTimeout: (fn: () => void) => {
      timers.push(fn)
      return timers.length
    },
    addEventListener: (type: string, fn: (event: unknown) => void) => {
      handlers[type] = fn
    },
    __TAURI_INTERNALS__: {
      invoke: (cmd: string, args: unknown) => {
        invokes.push({ cmd, args })
        return Promise.resolve()
      },
    },
  }
  sandbox.window = sandbox

  vm.createContext(sandbox)
  vm.runInContext(SCRIPT, sandbox)

  return {
    guard: sandbox.__phpStackBootGuard as Guard,
    host,
    timers,
    invokes,
    handlers,
    flushTimers: () => {
      const pending = timers.splice(0)
      pending.forEach((fn) => fn())
    },
  }
}

describe('boot-guard', () => {
  it('执行后挂上兜底 API', () => {
    const h = setup()
    expect(typeof h.guard.report).toBe('function')
    expect(typeof h.guard.markMounted).toBe('function')
    expect(typeof h.guard.entries).toBe('function')
  })

  describe('normalize', () => {
    it('message 缺失时回落到 error.message', () => {
      const h = setup()
      const out = h.guard.__test.normalize({ error: new Error('boom') })
      expect(out.message).toBe('boom')
    })

    it('message 缺失且无 error 时用 reason（unhandledrejection）', () => {
      const h = setup()
      const out = h.guard.__test.normalize({ reason: 'connection refused' })
      expect(out.message).toBe('connection refused')
    })

    it('完全无信息时给出占位文案而不是空串', () => {
      const h = setup()
      const out = h.guard.__test.normalize({})
      expect(out.message).toBe('(empty error message)')
      expect(out.source).toBe('unknown')
    })

    it('用 lineno/colno 拼出 location', () => {
      const h = setup()
      const out = h.guard.__test.normalize({
        filename: 'index.js',
        lineno: 12,
        colno: 34,
      })
      expect(out.location).toBe('index.js:12:34')
    })
  })

  describe('truncate', () => {
    it('非字符串入参不炸', () => {
      const h = setup()
      expect(h.guard.__test.truncate(null, 10)).toBe('')
      expect(h.guard.__test.truncate(undefined, 10)).toBe('')
      expect(h.guard.__test.truncate(42, 10)).toBe('42')
    })
  })

  describe('report', () => {
    it('按 message+source+location 去重，同一错误只上报一次', () => {
      const h = setup()
      h.guard.report({ message: 'same', source: 'window.onerror' })
      h.guard.report({ message: 'same', source: 'window.onerror' })
      expect(h.guard.entries()).toHaveLength(1)
      expect(h.invokes).toHaveLength(1)
    })

    it('上报载荷形状与 Rust FrontendErrorReport 对齐', () => {
      const h = setup()
      h.guard.report({ message: 'x', source: 'vue:render', location: 'a:1:1' })
      expect(h.invokes[0].cmd).toBe('log_frontend_error')
      expect(h.invokes[0].args).toEqual({
        report: { message: 'x', source: 'vue:render', stack: '', location: 'a:1:1' },
      })
    })

    it('超过 5 条后仍记录但不再上报，避免刷屏', () => {
      const h = setup()
      for (let i = 0; i < 8; i++) {
        h.guard.report({ message: 'err-' + i, source: 's' })
      }
      expect(h.guard.entries()).toHaveLength(8)
      expect(h.invokes).toHaveLength(5)
    })

    it('invoke 不可用时不抛异常（Tauri 未注入 / 加载失败）', () => {
      const bare: Record<string, unknown> = {
        document: { readyState: 'complete', getElementById: () => null, createElement },
        console: { error: () => {} },
        setTimeout: () => 0,
        addEventListener: () => {},
      }
      bare.window = bare
      vm.createContext(bare)
      expect(() => vm.runInContext(SCRIPT, bare)).not.toThrow()
      const g = bare.__phpStackBootGuard as Guard
      expect(() => g.report({ message: 'no tauri' })).not.toThrow()
      expect(g.entries()).toHaveLength(1)
    })
  })

  describe('事件钩子', () => {
    it('资源加载失败（script/link 404）必须被捕获 —— 白屏首因', () => {
      const h = setup()
      h.handlers.error({
        target: { tagName: 'SCRIPT', src: '/assets/index-deadbeef.js' },
      })
      const entry = h.guard.entries()[0]
      expect(entry.source).toBe('resource')
      expect(entry.message).toContain('/assets/index-deadbeef.js')
    })

    it('JS 异常走 window.onerror 分支', () => {
      const h = setup()
      const err = new Error('nope')
      h.handlers.error({ message: 'nope', error: err, filename: 'a.js', lineno: 1, colno: 2 })
      const entry = h.guard.entries()[0]
      expect(entry.source).toBe('window.onerror')
      expect(entry.location).toBe('a.js:1:2')
    })

    it('unhandledrejection 取 reason', () => {
      const h = setup()
      h.handlers.unhandledrejection({ reason: 'docker not running' })
      expect(h.guard.entries()[0]).toMatchObject({
        source: 'unhandledrejection',
        message: 'Unhandled promise rejection',
      })
    })
  })

  describe('白屏兜底面板', () => {
    it('#app 为空时渲染错误面板', () => {
      const h = setup()
      h.guard.report({ message: 'bundle exploded', source: 'window.onerror' })
      h.flushTimers()
      expect(h.host.appended).toHaveLength(1)
    })

    it('已挂载（#app 有子节点）则不覆盖界面', () => {
      const h = setup()
      h.host.childElementCount = 1
      h.guard.report({ message: 'late error', source: 'vue:render' })
      h.flushTimers()
      expect(h.host.appended).toHaveLength(0)
    })

    it('markMounted 之后不再渲染面板', () => {
      const h = setup()
      h.guard.markMounted()
      expect(h.guard.__test.isMounted()).toBe(true)
      h.guard.report({ message: 'after mount', source: 'vue:render' })
      h.flushTimers()
      expect(h.host.appended).toHaveLength(0)
    })

    it('没有任何错误事件时也会给出「未捕获到错误」提示', () => {
      const h = setup()
      h.flushTimers()
      expect(h.host.appended).toHaveLength(1)
      const panel = h.host.appended[0] as { children: Array<{ textContent: string }> }
      const text = panel.children.map((c) => c.textContent).join('\n')
      expect(text).toContain('Application failed to start')
      expect(text).toContain('com.php-stack.dev')
    })
  })
})
