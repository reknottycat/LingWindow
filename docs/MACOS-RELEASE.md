# macOS 发行构建与路径隐私

发行构建使用包内资源、显式模型目录和用户模型目录。源码树中的 assets/models 仅供 debug 构建回退；release 不依赖构建机器上的目录。

在已准备好既有依赖和模型资源的工作区执行：

```sh
node scripts/build-macos-release.mjs --config '{"bundle":{"macOS":{"signingIdentity":"-"}}}' -- --locked
```

该入口只为子进程动态设置 Rust 与 Clang/C++ 路径映射，不修改全局环境、Cargo 配置或模型。HOME、CARGO_HOME 与工程目录映射为通用的 /build 路径，保留 panic 源文件定位而不嵌入构建用户名。已有普通/编码 Rust flags 以及 CFLAGS/CXXFLAGS 会保留；两种 Rust 变量同时存在（包括显式空串）或已有路径映射时明确拒绝，需构建者先核对冲突。入口拒绝 --debug/-d 与任何 --profile 切换，仅使用默认 release 构建；其他 Tauri/Cargo 参数可原样附加。为使 cc-rs 与 CMake 正确解释含空格或引号的目录，子进程启用 CC_SHELL_ESCAPED_FLAGS；若调用环境显式关闭该设置则报冲突。原环境未启用 shell 解析时，先按 cc-rs 的 ASCII 空白分词规则拆分已有 CFLAGS/CXXFLAGS/ARFLAGS/RANLIBFLAGS（含目标与 HOST/TARGET 变体），逐 token 引用后传入，保留宏参数中的原有引号；原环境已启用时保留其原串。C/C++ 映射用于移除原生库 __FILE__ 中的本机构建路径。官方预编译依赖仍可能保留其公开 CI 路径，应在产物审计中与本机路径区分。

运行 `node --test scripts/test-build-paths.mjs` 检查入口，并用 release 模式运行原生测试以覆盖源码模型回退关闭。debug 测试确认开发回退仍保留。构建后仍须扫描 App 全部文件里的构建目录、用户目录及原生库来源路径，核资源哈希和 `codesign --verify --deep --strict`；路径映射不能代替最终产物审计。

上例使用 ad-hoc 签名，不是 Developer ID 签名或 Apple 公证。构建、单元测试和路径审计不能替代新二进制的实际界面、音频或硬件验收。不要直接替换已安装应用或已发布文件；先保留旧包和完整校验记录。
