# LingWindow Windows HYMT2 云端构建与验证

## 当前验证边界

本文件说明构建方法，不代表 Windows 构建已成功。只有对应提交的 Windows CI 成功日志和安装包，才能证明该提交在 Windows x64 上通过编译、单元测试和打包。Linux 上的源码检查或前端构建不能替代 Windows 验证。

标准 GitHub 托管 Windows runner 不用于验证 NVIDIA GPU。CI 不下载 HYMT2 大模型、不安装 CUDA 推理环境，也不声称验证 GPU 加速、实际翻译速度或显存占用。Python worker 单元测试验证协议和错误处理，不代表真实模型推理成功。

安装包保留上游基础 ASR/翻译模型。HYMT2 模型与 Python/Transformers/CUDA 运行环境由用户按单独的运行时安装说明配置，不打入此安装包。模型许可及其再分发条件需分别遵守。

## 云端构建

1. 将代码放入已获授权的专用 GitHub 仓库，启用 Actions。不要推送到上游或借用无关仓库。
2. 推送到 `main`、`windows-hymt2`、`windows-hymt2/…` 或 `lingwindow/…` 分支，或手动运行 `LingWindow Windows validation`。手动触发入口通常要求工作流先存在于默认分支。
3. CI 使用 `windows-latest` 的 x64 MSVC 环境，安装 Node 22、仓库声明的 pnpm、Python 3.12、Rust stable、typeshare-cli 1.13.4。
4. 保留 `RUSTFLAGS=-C target-feature=+crt-static`，使 Rust 与 CT2/sherpa 的静态 CRT 配置一致。
5. 安装前端依赖，获取基础模型，检查模型目录清单元数据，生成 TypeScript 类型并执行类型检查、Python worker 测试、前端构建、Rust 库测试。
6. 用 `pnpm tauri build --bundles nsis,msi` 生成安装包。不加载上游 updater 构建配置，不要求上游签名密钥。
7. 从该次运行下载安装包 artifact 与 evidence artifact。后者包含提交号、运行链接、系统/工具链信息、测试日志、构建日志与安装包 SHA256。

`pnpm verify:models` 检查 catalog 元数据，不是对所有已下载模型逐个计算校验和。各下载脚本自己的校验逻辑仍然适用。安装包的 SHA256 用于核对传输完整性，不等于发行者数字签名。

工作流仅有分支 push 与手动触发，无 tag 触发、Release 创建或部署步骤，且仓库权限为 `contents: read`。原上游 `build.yml`、`pages.yml` 已保留为 `.disabled`，不会触发旧签名、发布或 Pages 部署。正式发布由维护者单独审核，验证工作流不发布 Release。

## 本地 Windows 重现

需要 x64 Windows、Visual Studio C++ Build Tools 与 Windows SDK、Rust MSVC 工具链、Node 22、pnpm 10.29.2、Python 3.12，以及可联网下载依赖和基础模型的环境。打包工具由 Tauri 按需获取。

在 PowerShell 中运行：

```powershell
$env:RUSTFLAGS = '-C target-feature=+crt-static'
cargo install typeshare-cli --version 1.13.4 --locked
pnpm install --frozen-lockfile
pnpm fetch:mt
pnpm verify:models
pnpm gen:types
pnpm typecheck
python -X utf8 -m unittest discover -s scripts -p 'test_windows_hymt2_worker.py'
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --release --locked --lib
pnpm tauri build --bundles nsis,msi
```

每条命令成功后再继续。输出目录：

- `src-tauri/target/release/bundle/nsis/`：NSIS `.exe` 安装包。
- `src-tauri/target/release/bundle/msi/`：MSI 安装包。

## 真机验收仍需完成

- 在目标 Windows 机器安装、启动、卸载，确认 WebView2 和运行时依赖可用。
- 分别验证系统声音、麦克风、多声道麦克风回退和混音。
- 验证悬浮字幕、置顶、托盘、全局快捷键与退出后的资源回收。
- 配置真实 HYMT2 模型和 Python 环境，验证启动、翻译、取消、重启与缺失依赖的错误提示。
- 如使用 NVIDIA GPU，单独确认驱动、CUDA 兼容性、实际设备选择、显存与延迟；不能用 CPU 测试或 CI 成功替代。
- 定制版本应禁用上游自动更新或改成自己的受控更新来源，防止被原版覆盖。不生成 updater artifacts 本身并不会禁用应用内更新查询。

CI 单元测试和打包不覆盖以上交互场景。未签名安装包可能触发 Windows SmartScreen；正式发行前应另行安排受信任代码签名，而不是向本流程复制上游密钥。

应用标识为 `com.lingwindow.desktop`；实际可执行文件名以 `Cargo.toml` 的 `[[bin]].name` 为准，当前是 `LingWindow.exe`。工作流读取并校验该名称，不沿用旧 `AudioScript.exe`。包内保留 `NOTICE` 及 `resources/third-party` 的来源和许可。
