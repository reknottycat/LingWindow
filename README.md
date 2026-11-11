# LingWindow · 聆窗

[简体中文](README.zh-CN.md) · [Privacy](docs/PRIVACY.md) · [Third-party notices](src-tauri/resources/third-party/THIRD_PARTY_NOTICES.md)

Local speech transcription, translation and floating captions for macOS and Windows, with a focus on Russian and English to Chinese.

LingWindow is an independent derivative of [AudioScript](https://github.com/niyongsheng/AudioScript) **v0.7.0**. Thank you to **Nico (niyongsheng)** and all upstream contributors for sharing their work. The original copyright and MIT license are retained. This is not an official AudioScript release and does not imply upstream endorsement.

Upstream baseline: [`c38becd4556e07791b600bff0d73b32c2c4ae06c`](https://github.com/niyongsheng/AudioScript/commit/c38becd4556e07791b600bff0d73b32c2c4ae06c).

## Features

- System audio, microphone, mixed audio and selected-application capture, subject to platform support and permissions. Level preview requires an explicit click; opening settings does not start capture.
- AudioScript's existing recognition pipeline, including optional Nemotron 3.5 and Qwen3-ASR models. Russian requires a Russian-capable model; the bundled Chinese/English Zipformer does not support Russian.
- Local HY-MT2 translation: MLX 4-bit on Apple Silicon; a separate Python / Transformers runtime with optional NVIDIA CUDA on Windows. The current adapter supports Russian or English to Chinese.
- Existing LMT, OPUS-MT and other translation choices remain available. Their capabilities and licenses differ.
- Original, bilingual and translation-only caption modes, a visible toolbar, hide control, drag region and saved display preferences.
- A silent text test uses the actual translation queue and overlay without playback, capture or ASR. It checks translation setup, not speech end-to-end performance.
- Thirteen interface languages and a separate application identity, `com.lingwindow.desktop`.

## Installation and models

Use only packages, platform labels and checksums actually published in [this project's repository](https://github.com/reknottycat/AudioScript-Windows). Source availability or a running CI job does not mean an installer has been released. AudioScript's original downloads are not LingWindow updates.

- **macOS:** Apple Silicon, macOS 14.2 or later. HY-MT2 needs a separate local MLX model and runtime. See [Mac setup](docs/HYMT2-LOCAL.md).
- **Windows:** x64. See [setup](docs/WINDOWS-START-HERE.zh-CN.md) and [build/validation](docs/WINDOWS-HYMT2-BUILD.md). Unavailable CUDA produces an error. CPU mode must be selected explicitly and is not guaranteed to be real-time.
- Model weights and Python environments are excluded from the source repository. Packages may include basic Chinese/English models; optional HY-MT2 and Nemotron models are installed separately.
- Verified domestic model sources are preferred. Downloads require a network connection; local recognition and translation can run offline once the required assets are installed.
- A new application identity can require new operating-system recording permissions. Keep the original application and data; migration must not silently remove or overwrite them.

Personal builds may be ad-hoc signed or unsigned and may not be notarized. A checksum verifies file integrity, not software safety. Do not disable system-wide protection to run an unknown copy.

## Privacy and updates

This derivative disables Aptabase and Sentry reporting. Old analytics preferences cannot enable reporting. Audio, captions and diagnostic logs are not automatically uploaded. Model downloads and explicitly opened web pages still use the network. See [privacy details](docs/PRIVACY.md).

LingWindow does not use upstream update endpoints or signing keys. Updates are distributed manually through this project's own release channel.

## Development

Node.js 22, pnpm 10.29.2, Rust and the target platform's build tools are required. Initial dependency and model setup needs network access and disk space.

```sh
pnpm install --frozen-lockfile
pnpm fetch:mt
pnpm gen:types
pnpm typecheck
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --release --locked --lib
pnpm tauri build
```

See the [HY-MT2 document](docs/HYMT2-LOCAL.md) for Apple Silicon toolchain and runtime details. Follow the Windows-specific instructions for MSVC, static CRT, Python and CUDA requirements.

## Validation limits

- Compilation, unit tests, protocol tests, actual model inference, desktop interaction and speech end-to-end tests are separate stages. Consult the evidence attached to the particular release.
- Text-to-caption timing excludes speech duration, segmentation and ASR. It is not speech end-to-end latency.
- Hosted Windows CI does not validate a physical GPU, system capture, global hotkeys or visible overlay pixels.
- Dragging, fullscreen coverage, permissions and per-application audio depend on the operating system and desktop environment. Unchecked hardware behavior must remain labeled unverified.

## License and acknowledgments

Application source is [MIT licensed](LICENSE), with the original attribution retained in [NOTICE](NOTICE). Thanks to AudioScript, Tauri, sherpa-onnx, MLX, CTranslate2 and the model maintainers.

Models and dependencies keep their own licenses. In particular, the OPUS Chinese-to-English model requires CC-BY-4.0 attribution, and the optional NLLB model carries CC-BY-NC-4.0 restrictions. Review the [third-party notices](src-tauri/resources/third-party/THIRD_PARTY_NOTICES.md) and relevant model cards before redistribution or use.
