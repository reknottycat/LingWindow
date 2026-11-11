import assert from 'node:assert/strict'
import test from 'node:test'
import { releaseBuildArgs, releaseBuildEnv } from './build-macos-release.mjs'

const roots = { home: '/fixture/home', cargoHome: '/fixture/home/cargo', project: '/fixture/home/project with spaces' }

test('动态路径映射支持空格，且不改变父进程环境', () => {
  const input = { HOME: roots.home, OTHER: 'keep' }
  const env = releaseBuildEnv(input, roots)
  assert.deepEqual(input, { HOME: roots.home, OTHER: 'keep' })
  assert.equal(env.OTHER, 'keep')
  assert.deepEqual(env.CARGO_ENCODED_RUSTFLAGS.split('\x1f'), [
    '--remap-path-prefix=/fixture/home=/build/home',
    '--remap-path-prefix=/fixture/home/cargo=/build/cargo',
    '--remap-path-prefix=/fixture/home/project with spaces=/build/lingwindow',
  ])
})

test('保留普通 Rust 编译参数', () => {
  const env = releaseBuildEnv({ RUSTFLAGS: '-C target-cpu=native' }, roots)
  assert.deepEqual(env.CARGO_ENCODED_RUSTFLAGS.split('\x1f').slice(0, 2), ['-C', 'target-cpu=native'])
  assert.equal(env.RUSTFLAGS, undefined)
})

test('保留已编码的 Rust 参数', () => {
  const env = releaseBuildEnv({ CARGO_ENCODED_RUSTFLAGS: '-C\x1flink-arg=some path' }, roots)
  assert.deepEqual(env.CARGO_ENCODED_RUSTFLAGS.split('\x1f').slice(0, 2), ['-C', 'link-arg=some path'])
})

test('明确拒绝两种参数并存或已有路径映射冲突', () => {
  assert.throws(() => releaseBuildEnv({ RUSTFLAGS: '-C opt-level=3', CARGO_ENCODED_RUSTFLAGS: '-C\x1fdebuginfo=0' }, roots))
  assert.throws(() => releaseBuildEnv({ RUSTFLAGS: '--remap-path-prefix=/old=/new' }, roots))
  assert.throws(() => releaseBuildEnv({ CARGO_ENCODED_RUSTFLAGS: '--remap-path-prefix\x1f/old=/new' }, roots))
})

test('拒绝相对或含映射分隔符的目录', () => {
  assert.throws(() => releaseBuildEnv({}, { ...roots, home: 'relative' }))
  assert.throws(() => releaseBuildEnv({}, { ...roots, cargoHome: '/bad=path' }))
})

test('显式空编码变量仍算已设置，不悄悄采用另一种 flags', () => {
  const env = releaseBuildEnv({ CARGO_ENCODED_RUSTFLAGS: '' }, roots)
  assert.equal(env.CARGO_ENCODED_RUSTFLAGS.split('\x1f').length, 3)
  assert.throws(() => releaseBuildEnv({ CARGO_ENCODED_RUSTFLAGS: '', RUSTFLAGS: '-C opt-level=3' }, roots))
  assert.throws(() => releaseBuildEnv({ CARGO_ENCODED_RUSTFLAGS: '', RUSTFLAGS: '' }, roots))
})

test('原生编译参数保留且路径映射正确转义空格与引号', () => {
  const env = releaseBuildEnv({ CFLAGS: '-O2', CXXFLAGS: '-DNDEBUG' }, roots)
  assert.ok(env.CFLAGS.startsWith("'-O2' "))
  assert.ok(env.CXXFLAGS.startsWith("'-DNDEBUG' "))
  assert.ok(env.CXXFLAGS.includes("'-ffile-prefix-map=/fixture/home/project with spaces=/build/lingwindow'"))
  const quoted = releaseBuildEnv({}, { ...roots, project: "/fixture/user's project" })
  assert.ok(quoted.CXXFLAGS.includes("user'\\''s project"))
  assert.throws(() => releaseBuildEnv({ CXXFLAGS: '-ffile-prefix-map=/old=/new' }, roots))
  assert.throws(() => releaseBuildEnv({ CFLAGS: '-fdebug-prefix-map=/old=/new' }, roots))
})

test('发行入口拒绝 debug 与自定义 profile，其他参数透传', () => {
  for (const args of [['--debug'], ['-d'], ['--debug=true'], ['--', '--profile', 'dev'], ['--profile=release']]) {
    assert.throws(() => releaseBuildArgs(args))
  }
  const args = ['--config', '{}', '--', '--locked', '-j6']
  assert.deepEqual(releaseBuildArgs(args), ['tauri', 'build', '--bundles', 'app', ...args])
})


test('cc-rs 使用 shell 参数解析且不覆盖显式关闭', () => {
  assert.equal(releaseBuildEnv({}, roots).CC_SHELL_ESCAPED_FLAGS, '1')
  assert.equal(releaseBuildEnv({ CC_SHELL_ESCAPED_FLAGS: 'true' }, roots).CC_SHELL_ESCAPED_FLAGS, '1')
  for (const value of ['', '0', 'false', 'no']) {
    assert.throws(() => releaseBuildEnv({ CC_SHELL_ESCAPED_FLAGS: value }, roots))
  }
})


test('启用 shell 解析前按 cc-rs 原语义保留宏引号和目标 flags', () => {
  const env = releaseBuildEnv({ CFLAGS: '-DNAME="foo" -O2', HOST_CXXFLAGS: '-DMSG="a b"', ARFLAGS: 'crs' }, roots)
  assert.ok(env.CFLAGS.startsWith("'-DNAME=\"foo\"' '-O2' "))
  assert.equal(env.HOST_CXXFLAGS, "'-DMSG=\"a' 'b\"'")
  assert.equal(env.ARFLAGS, "'crs'")
  const shell = releaseBuildEnv({ CC_SHELL_ESCAPED_FLAGS: '1', CFLAGS: '-DNAME="foo" -O2', HOST_CXXFLAGS: '-DMSG="a b"' }, roots)
  assert.ok(shell.CFLAGS.startsWith('-DNAME="foo" -O2 '))
  assert.equal(shell.HOST_CXXFLAGS, '-DMSG="a b"')
  assert.throws(() => releaseBuildEnv({ CFLAGS_aarch64_apple_darwin: '-ffile-prefix-map=/old=/new' }, roots))
})
