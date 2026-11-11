import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import vm from 'node:vm'
import { createRequire } from 'node:module'
import { test } from 'node:test'
import ts from 'typescript'

const require = createRequire(import.meta.url)
const root = path.resolve(import.meta.dirname, '..')
function harness() {
  let slots = [], index = 0, queued = [], startCalls = [], stopCalls = 0
  let store = { audioSource: 'loopback', devices: [], device: '', mixMicDevices: [], mixSystemDevices: [], mixMicDevice: '', mixSystemDevice: '', appTargets: [], audioProcesses: [], processesLoading: false, processError: null, deviceError: null, audio: null, status: 'ready', pending: null, prefsReady: true, devicesLoading: false }
  const same = (a, b) => a && b && a.length === b.length && a.every((x, i) => Object.is(x, b[i]))
  const react = {
    useState(value) { const at = index++; if (!(at in slots)) slots[at] = typeof value === 'function' ? value() : value; return [slots[at], v => { slots[at] = typeof v === 'function' ? v(slots[at]) : v }] },
    useRef(value) { const at = index++; return slots[at] ??= { current: value } },
    useMemo(fn, deps) { const at = index++; if (!same(slots[at]?.deps, deps)) slots[at] = { deps, value: fn() }; return slots[at].value },
    useCallback(fn, deps) { return this.useMemo(() => fn, deps) },
    useEffect(fn, deps) { const at = index++; if (!same(slots[at]?.deps, deps)) { const old = slots[at]; slots[at] = { deps, cleanup: old?.cleanup }; queued.push(() => { old?.cleanup?.(); slots[at].cleanup = fn() }) } },
  }
  react.useCallback = (fn, deps) => react.useMemo(() => fn, deps)
  const ipc = { AudioSourceKind: { Loopback: 'loopback', Mic: 'mic', Mix: 'mix', App: 'app' }, formatCmdError: String, permissionOf: () => null, monitorStart: async targets => { startCalls.push(targets) }, monitorStop: async () => { stopCalls++ } }
  const components = {}
  const load = name => {
    if (name === 'react') return react
    if (name === 'react/jsx-runtime') return require(name)
    if (name === '@/main/store') return { useStore: () => store }
    if (name === '@/shared/i18n') return { useI18n: () => ({ t: key => key }) }
    if (name === '@/shared/ipc') return ipc
    if (name === '@/shared/types') return { TaskStatus: { Ready: 'ready', Capturing: 'capturing' } }
    if (name.startsWith('@/ui/')) return new Proxy({}, { get: (_, key) => components[key] ??= Object.assign(() => null, { displayName: key }) })
    throw new Error(`不允许加载实际原生服务：${name}`)
  }
  const file = path.join(root, 'src/main/pages/AudioPage.tsx')
  const code = ts.transpileModule(fs.readFileSync(file, 'utf8'), { compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX } }).outputText
  const module = { exports: {} }
  vm.runInNewContext(`(function(require,module,exports){${code}\n})`, { console, window: { addEventListener() { throw Error('页面不能注册自动聚焦采集') }, removeEventListener() {} } })(load, module, module.exports)
  const render = () => { index = 0; const tree = module.exports.AudioPage({ onOpenPermissions() {} }); const effects = queued; queued = []; effects.forEach(fn => fn()); return tree }
  const button = (tree, label) => {
    if (!tree || typeof tree !== 'object') return null
    if (tree.type === components.Button && tree.props.children === label) return tree.props
    for (const item of Object.values(tree.props ?? tree)) { if (Array.isArray(item)) { for (const child of item) { const got = button(child, label); if (got) return got } } else { const got = button(item, label); if (got) return got } }
    return null
  }
  return { render, button, set: patch => Object.assign(store, patch), starts: () => startCalls.length, stops: () => stopCalls, unmount: () => slots.forEach(s => s?.cleanup?.()) }
}
const settle = () => new Promise(resolve => setImmediate(resolve))
test('挂载和重复渲染不启动音量采集', () => { const h = harness(); h.render(); h.render(); assert.equal(h.starts(), 0); h.unmount(); assert.equal(h.starts(), 0) })
test('只有显式点击才启动，重复点击不重复采集，停止后不重启', async () => { const h = harness(); let tree = h.render(); const start = h.button(tree, 'audio.previewStart'); assert.ok(start); assert.equal(start.disabled, false); start.onClick(); start.onClick(); assert.equal(h.starts(), 1); await settle(); tree = h.render(); h.button(tree, 'audio.previewStop').onClick(); h.render(); assert.equal(h.starts(), 1); assert.ok(h.stops() > 0); h.unmount() })
test('切换音源和离开页面会停止且不会自动重启', async () => { const h = harness(); h.button(h.render(), 'audio.previewStart').onClick(); await settle(); h.render(); const n = h.stops(); h.set({ audioSource: 'mic' }); h.render(); assert.ok(h.stops() > n); assert.equal(h.starts(), 1); h.render(); assert.equal(h.starts(), 1); h.unmount(); assert.equal(h.starts(), 1) })
test('会话采集、启动中和未就绪时不能打开预览', () => { for (const patch of [{ status: 'capturing' }, { pending: 'start' }, { prefsReady: false }, { devicesLoading: true }, { audioSource: 'app', appTargets: [] }]) { const h = harness(); h.set(patch); const start = h.button(h.render(), 'audio.previewStart'); assert.equal(start.disabled, true); start.onClick(); assert.equal(h.starts(), 0); h.unmount() } })
