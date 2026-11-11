import { spawnSync } from 'node:child_process'
import { homedir } from 'node:os'
import { dirname, isAbsolute, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

export function releaseBuildEnv(env, { home, cargoHome, project }) {
  const encodedPresent = Object.hasOwn(env, 'CARGO_ENCODED_RUSTFLAGS')
  if (encodedPresent && Object.hasOwn(env, 'RUSTFLAGS')) {
    throw new Error('请仅设置 CARGO_ENCODED_RUSTFLAGS 或 RUSTFLAGS，避免忽略已有编译参数。')
  }
  const flags = encodedPresent
    ? (env.CARGO_ENCODED_RUSTFLAGS ? env.CARGO_ENCODED_RUSTFLAGS.split('\x1f') : [])
    : (env.RUSTFLAGS ?? '').trim().split(/\s+/).filter(Boolean)
  if (flags.some((flag) => flag.startsWith('--remap-path-prefix'))) {
    throw new Error('已有路径映射参数；请先确认映射规则，避免本脚本覆盖现有规则。')
  }
  for (const path of [home, cargoHome, project]) {
    if (!isAbsolute(path) || /[=\x1f\r\n]/.test(path)) {
      throw new Error('构建目录必须是绝对路径，且不能包含映射分隔符。')
    }
  }
  const mapped = [
    ...flags,
    `--remap-path-prefix=${home}=/build/home`,
    `--remap-path-prefix=${cargoHome}=/build/cargo`,
    `--remap-path-prefix=${project}=/build/lingwindow`,
  ]
  const result = { ...env, CARGO_ENCODED_RUSTFLAGS: mapped.join('\x1f') }
  delete result.RUSTFLAGS
  // 原生库的 __FILE__ 不受 Rust 映射影响；仅为本次构建补充 Clang 路径映射。
  const quote = (value) => `'${value.replaceAll("'", "'\\''")}'`
  if (Object.hasOwn(env, 'CC_SHELL_ESCAPED_FLAGS')
    && ['', '0', 'false', 'no'].includes(env.CC_SHELL_ESCAPED_FLAGS.toLowerCase())) {
    throw new Error('CC_SHELL_ESCAPED_FLAGS 已显式关闭；路径引用需要启用 shell 参数解析。')
  }
  const nativeShellWasEnabled = Object.hasOwn(env, 'CC_SHELL_ESCAPED_FLAGS')
  // cc-rs 默认按 ASCII 空白分词；先保留旧 token，再启用 shell 引用。
  for (const key of Object.keys(env)) {
    if (!/^(?:(?:HOST|TARGET)_)?(?:CFLAGS|CXXFLAGS|ARFLAGS|RANLIBFLAGS)(?:_.+)?$/.test(key)) continue
    if (/-f(?:file|macro|debug)-prefix-map/.test(env[key])) {
      throw new Error(`已有 ${key} 路径映射参数；请先核对规则。`)
    }
    if (!nativeShellWasEnabled) {
      result[key] = env[key].split(/[\t\n\v\f\r ]+/).filter(Boolean).map(quote).join(' ')
    }
  }
  result.CC_SHELL_ESCAPED_FLAGS = '1'
  const nativeMappings = [
    `-ffile-prefix-map=${home}=/build/home`,
    `-ffile-prefix-map=${cargoHome}=/build/cargo`,
    `-ffile-prefix-map=${project}=/build/lingwindow`,
  ].map(quote).join(' ')
  for (const key of ['CFLAGS', 'CXXFLAGS']) {
    result[key] = [result[key], nativeMappings].filter(Boolean).join(' ')
  }
  return result
}

export function releaseBuildArgs(args) {
  if (args.some((arg) => arg === '-d' || arg === '--debug' || arg.startsWith('--debug=')
    || arg === '--profile' || arg.startsWith('--profile='))) {
    throw new Error('此入口仅支持默认 release 构建，请移除 debug/profile 切换参数。')
  }
  return ['tauri', 'build', '--bundles', 'app', ...args]
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  if (process.platform !== 'darwin') throw new Error('此构建入口仅用于 macOS。')
  const project = resolve(dirname(fileURLToPath(import.meta.url)), '..')
  const home = homedir()
  const cargoHome = resolve(process.env.CARGO_HOME || resolve(home, '.cargo'))
  const env = releaseBuildEnv(process.env, { home, cargoHome, project })
  const result = spawnSync('pnpm', releaseBuildArgs(process.argv.slice(2)), {
    cwd: project,
    env,
    stdio: 'inherit',
  })
  if (result.error) throw result.error
  process.exitCode = result.status ?? 1
}
