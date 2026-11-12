import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { createRequire } from 'node:module'
import { test } from 'node:test'
import vm from 'node:vm'
import ts from 'typescript'
import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'

// 无浏览器、音频或模型依赖；用项目现有 TypeScript 编译器渲染真实组件。
const root = path.resolve(import.meta.dirname, '..')
const require = createRequire(import.meta.url)
const cache = new Map()
let locale = 'en'
const loads = []
function load(name, from = path.join(root, 'src')) {
  if (name === '@tauri-apps/api/window') return { getCurrentWindow: () => { throw new Error('测试不可调用原生窗口') } }
  if (name === '@/shared/ipc') return { formatCmdError: String }
  if (name === '@/shared/i18n') return { useI18n: () => ({ t: (key, vars = {}) => {
    const messages = locale === 'zh-CN' ? load('@/shared/i18n/zh-CN').zhCN : load('@/shared/i18n/en').en
    return (messages[key] ?? key).replace(/\{(\w+)\}/g, (match, key) => vars[key] ?? match)
  } }) }
  if (!name.startsWith('@/') && !name.startsWith('.')) return require(name)
  const base = name.startsWith('@/') ? path.join(root, 'src', name.slice(2)) : path.resolve(from, name)
  const file = [base, `${base}.tsx`, `${base}.ts`, path.join(base, 'index.ts')].find((p) => fs.existsSync(p) && fs.statSync(p).isFile())
  if (!file) throw new Error(`测试模块不存在：${base}`)
  if (cache.has(file)) return cache.get(file).exports
  const module = { exports: {} }
  cache.set(file, module)
  const code = ts.transpileModule(fs.readFileSync(file, 'utf8'), { compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX, target: ts.ScriptTarget.ES2022 } }).outputText
  vm.runInThisContext(`(function(require, module, exports) {${code}\n})`, { filename: file })((name) => load(name, path.dirname(file)), module, module.exports)
  loads.push(file)
  return module.exports
}
const { StreamView } = load('@/overlay/StreamView')
const { OverlayToolbar } = load('@/overlay/OverlayToolbar')
const baseSegment = { id: 'test:caption', text: 'Original sample', translation: '译文样本', translationState: 'done', state: 'final', revision: 1, startMs: 0, endMs: 10 }
const render = (displayMode, segment = baseSegment) => renderToStaticMarkup(React.createElement(StreamView, { segments: [segment], displayMode, fontSize: 22, fontColor: '#ffffff', opacity: 60, bgColor: '#101018', glow: false, glowSpeed: 0, glowWidth: 2, streaming: false }))
const shortcuts = [{ id: 'overlay.toggleMode', acceleratorPretty: '⌘⇧K', registered: true }, { id: 'overlay.toggleVisible', acceleratorPretty: '⌘⇧J', registered: true }]
const toolbar = (mode = 'edit', keys = shortcuts, busy = false) => renderToStaticMarkup(React.createElement(OverlayToolbar, { config: { mode, displayMode: 'bilingual' }, shortcuts: keys, busy, onHide() {}, onLock() {}, onDisplayMode() {}, onError() {} }))

test('双语同时显示原文与译文', () => {
  const html = render('bilingual')
  assert.match(html, /Original sample/)
  assert.match(html, /译文样本/)
  assert.match(html, /font-size:0.8em/)
})
test('原文模式使用完整字号且不显示译文或翻译错误', () => {
  const html = render('source')
  assert.match(html, /Original sample/)
  assert.match(html, /font-size:1em;opacity:1/)
  assert.doesNotMatch(html, /译文样本/)
  assert.doesNotMatch(render('source', { ...baseSegment, translationState: 'failed' }), /Translation failed/)
})
test('译文模式不泄露原文，含失败、等待与空译文状态', () => {
  assert.doesNotMatch(render('translation'), /Original sample/)
  assert.match(render('translation'), /译文样本/)
  for (const [state, expected] of [['pending', 'Translating…'], ['idle', 'Waiting for translation…'], ['failed', 'Translation failed'], ['done', 'No translation for this segment']]) {
    const html = render('translation', { ...baseSegment, translation: '', translationState: state })
    assert.match(html, new RegExp(expected))
    assert.doesNotMatch(html, /Original sample/)
  }
})
test('编辑工具栏包含拖动、三档、锁定与隐藏，并使用实际改绑快捷键', () => {
  const html = toolbar()
  assert.equal((html.match(/<button/g) ?? []).length, 6)
  for (const label of ['Move overlay', 'Source', 'Both', 'Translation', '⌘⇧K', '⌘⇧J', 'height:40px']) assert.ok(html.includes(label), label)
  assert.match(html, /aria-pressed="true"/)
  assert.doesNotMatch(html, /⌘⇧L|⌘⇧H/)
})
test('锁定只有状态与恢复提示，不呈现无法点击的按钮', () => {
  const html = toolbar('locked')
  assert.match(html, /Locked · ⌘⇧K to unlock/)
  assert.match(html, /main window → Overlay/)
  assert.doesNotMatch(html, /<button/)
  assert.match(toolbar('locked', []), /Unlock in the main window → Overlay/)
})
test('快捷键注册失败时禁用浮窗锁定，忙时禁用可变控件', () => {
  assert.match(toolbar('edit', []), /disabled=""/)
  assert.match(toolbar('edit', []), /working unlock shortcut/)
  assert.ok((toolbar('edit', shortcuts, true).match(/disabled=""/g) ?? []).length >= 3)
})
test('中文控件走本地化且没有硬编码快捷键', () => {
  locale = 'zh-CN'
  try {
    const html = toolbar()
    for (const text of ['拖动', '原文', '双语', '译文', '不会停止转写', '⌘⇧K']) assert.ok(html.includes(text), text)
  } finally { locale = 'en' }
})
test('渲染组件没有导入音频、ASR或翻译服务', () => {
  assert.equal(loads.some((p) => /session|audio|asr/i.test(path.relative(root, p))), false)
})


test('double click restores the main window without treating a click as a drag', () => {
  const overlay = fs.readFileSync(path.join(root, 'src/overlay/main.tsx'), 'utf8')
  const toolbar = fs.readFileSync(path.join(root, 'src/overlay/OverlayToolbar.tsx'), 'utf8')
  const commands = fs.readFileSync(path.join(root, 'src-tauri/src/commands/mod.rs'), 'utf8')
  const lifecycle = fs.readFileSync(path.join(root, 'src-tauri/src/lifecycle/mod.rs'), 'utf8')
  assert.match(overlay, /onDoubleClick/)
  assert.match(overlay, /config.mode === OverlayMode.Edit/)
  assert.match(toolbar, /onDoubleClick=\{onShowMain\}/)
  assert.match(toolbar, /Math.hypot[\s\S]*?< 4/)
  assert.match(commands, /"main_window_show"[\s\S]*?lifecycle::show_main/)
  assert.match(lifecycle, /unminimize\(\)/)
})


test('overlay exposes compact direction controls and safe runtime IPC', () => {
  const overlay = fs.readFileSync(path.join(root, 'src/overlay/main.tsx'), 'utf8')
  const control = fs.readFileSync(path.join(root, 'src/overlay/DirectionControls.tsx'), 'utf8')
  const toolbarSource = fs.readFileSync(path.join(root, 'src/overlay/OverlayToolbar.tsx'), 'utf8')
  assert.match(overlay, /directionControls={<DirectionControls/)
  assert.match(control, /sessionSetDirection/)
  assert.match(control, /sourceLanguage/)
  assert.match(control, /targetLanguage/)
  assert.match(control, /swapDirection/)
  assert.match(toolbarSource, /w-7 cursor-grab/)
  assert.doesNotMatch(control, /prefsSet|sessionStart|sessionStop/)
})
