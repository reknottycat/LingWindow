# 聆窗 LingWindow

[English](README.md) · [隐私说明](docs/PRIVACY.md) · [第三方许可](src-tauri/resources/third-party/THIRD_PARTY_NOTICES.md)

面向 macOS 与 Windows 的本地语音转写、多语种翻译和悬浮字幕应用。可用的语音翻译方向同时取决于所选识别模型和翻译模型。

聆窗基于 [AudioScript](https://github.com/niyongsheng/AudioScript) **v0.7.0**，感谢原作者 **Nico（niyongsheng）** 和所有贡献者开放源码。本项目保留原作者版权与 MIT 许可，是独立衍生版，不代表原作者或 AudioScript 官方。

上游基线：[`c38becd4556e07791b600bff0d73b32c2c4ae06c`](https://github.com/niyongsheng/AudioScript/commit/c38becd4556e07791b600bff0d73b32c2c4ae06c)。

## 功能

- 系统声音、麦克风、混合及指定应用采集，具体可用性取决于系统与权限。音量预览必须主动点击开始，浏览设置页不会自动采音。
- 沿用 AudioScript 的识别框架，可选择 Nemotron 3.5、Qwen3-ASR 等模型。俄语需要支持俄语的模型，内置中英 Zipformer 不支持俄语。
- HY-MT2 本地翻译：Apple Silicon 使用 MLX 4-bit；Windows 使用独立 Python / Transformers 运行环境，可配置 NVIDIA CUDA。当前源码支持 HY-MT2 目录中不同语言之间互译（37 个基础语言代码）。语音源语言取识别模型与翻译模型支持范围的交集，目标语言按翻译模型提供；保留中文简繁转换。可选方向不等于所有语言对都已实测。
- 保留 LMT、OPUS-MT 等原有翻译选项。模型能力与许可各自独立。
- 字幕浮窗提供原文、双语、译文三档显示，以及工具栏、隐藏、拖动区域和显示偏好保存。
- 无声文字测试走真实翻译队列与浮窗，不播放声音、不录音、不调用语音识别。它可以检查翻译配置，不能替代语音端到端测试。
- 13 种界面语言。独立应用标识 `com.lingwindow.desktop`，与原版的数据目录隔离。

## 安装与模型

以[本项目仓库](https://github.com/reknottycat/AudioScript-Windows)中实际发布的版本、平台与校验和为准。源码存在或 CI 启动不代表安装包已经发布。请不要把 AudioScript 官网的原版下载当成聆窗更新。

- **macOS**：Apple Silicon，macOS 14.2 或更新。HY-MT2 需要另外准备本地 MLX 模型和运行环境，见 [Mac 配置说明](docs/HYMT2-LOCAL.md)。
- **Windows**：x64，见 [安装与模型初始化](docs/WINDOWS-START-HERE.zh-CN.md)及[构建与验证](docs/WINDOWS-HYMT2-BUILD.md)。CUDA 不可用会明确报错；CPU 模式需显式选择，速度不保证实时。
- 模型与 Python 环境不放进源码仓库。安装包可包含基础中英语音/翻译模型；HY-MT2、Nemotron 等可选模型另行安装。
- 优先使用已核对文件与 SHA256 的国内模型源。下载仍需要网络；模型准备好后，本地识别与翻译可离线运行。
- 新应用可能需要重新申请操作系统录音权限。保留旧应用和数据，不自动删除或覆盖它们。

个人构建可能只有临时签名，或没有发行者签名、公证。校验和只能验证文件完整性，不能证明软件安全；不要关闭全局系统防护来运行未知副本。

## 隐私与更新

本版禁用 Aptabase 与 Sentry 上报，旧统计开关也不能启用收集。不会自动上传音频、字幕或诊断日志。主动下载模型、打开网页等操作仍会访问网络，完整边界见[隐私说明](docs/PRIVACY.md)。

本版不使用上游自动更新服务或签名公钥，更新通过自己的发布渠道手动进行。

## 开发

需要 Node.js 22、pnpm 10.29.2、Rust 和目标平台构建工具。初次获取依赖与模型需要网络和磁盘空间。

```sh
pnpm install --frozen-lockfile
pnpm fetch:mt
pnpm gen:types
pnpm typecheck
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --release --locked --lib
pnpm tauri build
```

Apple Silicon 的具体工具链、运行环境与构建配置见 [HY-MT2 文档](docs/HYMT2-LOCAL.md)。Windows 使用其专用说明，尤其注意 MSVC、静态 CRT、Python 和 CUDA 版本。

## 验证边界

Windows BF16/CUDA 已完成俄→中、英→中、中→英、日→英、俄→英的短文本推理检查，其中英→中也已在真实双语浮窗显示。这不等于语音端到端、全部语言对质量或更新后的 Mac 安装包验收。Mac 扩展语言处理已写入源码，但不属于上述 Windows 实测的覆盖范围。

- 编译、单元测试、协议测试、真实模型推理、桌面交互和语音端到端测试是不同阶段，以每个发布版本附带的记录为准。
- 文字提交到字幕的时间不包含说话、分句等待或语音识别，不标为语音端到端延迟。
- 托管 Windows CI 不验证真实 GPU、系统声音、快捷键或浮窗像素；不能据此承诺指定显卡性能。
- 拖动、全屏覆盖、权限与应用音频支持受操作系统和窗口环境影响。未完成的真机项目应明确保留为未验证。

## 许可与致谢

应用源码沿用 [MIT](LICENSE)，原作者署名保留在 [NOTICE](NOTICE)。感谢 AudioScript、Tauri、sherpa-onnx、MLX、CTranslate2 及所用模型的维护者。

模型和依赖不因本仓库的 MIT 许可而改变授权。特别是 OPUS 中文到英文模型需保留 CC-BY-4.0 署名，NLLB 的可选模型有 CC-BY-NC-4.0 限制。分发和使用前请查阅[第三方说明](src-tauri/resources/third-party/THIRD_PARTY_NOTICES.md)以及对应模型卡。
