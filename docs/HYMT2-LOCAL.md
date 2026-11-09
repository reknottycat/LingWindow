# HY-MT2 local build (AudioScript 0.6.2)

This local adaptation adds HY-MT2 1.8B MLX 4-bit as the default translation tier on Apple Silicon Macs. Existing saved model selections are preserved. OPUS-MT, NLLB and LMT remain available. This is a local build, not an upstream release.

## Local installation

Under the application's model search root, create `mt-hymt2`. Preserve the full verified MLX model directory, including `chat_template.jinja`, `config.json`, `model.safetensors`, `tokenizer.json` and `tokenizer_config.json`. Add `runtime.json`:

```json
{"python":"/absolute/path/to/venv/bin/python"}
```

The interpreter must already have compatible `mlx` and `mlx-lm` packages. The tested versions are MLX 0.32.4 and mlx-lm 0.32.2. The verified model uses `hunyuan_v1_dense` and its weight SHA-256 is `a9cecbc77050b86b2fa3df3a8941d34ed3585cdbcce07bd64b3da289d50537c6`. Preserve Tencent's model license and attribution. The catalog deliberately offers no download button. Status checks validate the runtime path, JSON files and actual safetensors offsets. They do not recompute the 961 MB weight hash on every settings refresh or claim that an arbitrary Python environment has been tested. Model initialization performs the real load and warmup and reports errors.

Only English or Russian to Chinese is implemented. Simplified/traditional Chinese conversion uses the application's existing converter. Russian audio still requires an ASR engine/model that supports Russian. The HY-MT2 translation worker does not perform speech recognition.

## Runtime

Rust launches the configured interpreter directly, without a shell, HTTP server or listening port. An embedded Python worker loads the model once and handles private JSON-lines stdin/stdout requests. Hugging Face and Transformers offline mode are enforced, `local_files_only=True`, and `trust_remote_code=False`. Imports and diagnostics cannot contaminate protocol stdout.

Generation uses the official single-user Chinese translation prompt, the tokenizer chat template, temperature 0.7, top-p 0.6, top-k 20, repetition penalty 1.05 and a seed of 42 + request ID. The repetition context limit is 262144. Inputs are limited to 2000 Unicode characters. Output-limit termination is an error, not a successful truncated translation. Cold initialization has a 90-second deadline and each translation a 60-second deadline. On protocol, timeout or translation failure, Rust terminates and reaps the worker; the next test/session creates a fresh worker. There is no automatic LMT fallback. App exit terminates the private worker.

The actual recording path and silent test path both use `session.rs`'s translator factory/cache and pre-created `translate-worker` queue. No ASR implementation was replaced.

## Silent verification

On the Models page, enter text under **Silent text test** and select **Test translation and overlay**. This requires a local translation model to be selected and refuses recording/cleanup states. The command does not invoke audio capture, microphone permission, playback or ASR. It preserves existing transcript text and appends an explicitly labeled test segment with a `preview:` ID. The real overlay is shown and receives the normal transcript event.

`eventMs` measures text submission through translation-event emission, including cold model loading when needed. The overlay separately acknowledges a committed animation frame in the local log. This acknowledgment is not a guarantee that pixels were physically visible: visually inspect the real overlay. Neither measurement is audio end-to-end latency.

Run input-only tests with `python3 scripts/test-hymt2-worker.py`. To exercise the real model without audio, use the configured Python interpreter with `scripts/test-hymt2-worker.py --model /absolute/path/to/mt-hymt2`. The optional `--output /absolute/path/report.json` saves structured evidence. Rust module tests cover Unicode, transport repetition, timeout, malformed response, process early exit/reaping, truncated weights and transcript preservation. Existing queue tests cover close/drain/rejected pushes.

For the full app build, run frontend type checking/build and Rust tests using the repository's normal toolchain. The local model adaptation only supplies Chinese and English new strings; other locales use the existing English fallback. Do not represent text-only verification as an audio end-to-end test.

## Reproducible M6 build notes

The verified local toolchain is Rust/Cargo 1.90.0, CMake 4.4.4, pnpm 10.29.2 and typeshare-cli 1.13.4. Rust dependencies were obtained through the rsproxy.cn mirror with the original Cargo.lock checksums retained. Built-in model assets were copied from the original installed application, not downloaded again. The sherpa-onnx 1.13.8 arm64 static library was obtained from the k2-fsa official GitHub release and verified against its release SHA-256 `9091bf160dc7fdacedbc906b212badf53c2993f4e5277a0e03998e96c31d60da`.

The current macOS 27.2 linker produced an invalid LINKEDIT string-pool alignment when stripping a build-time Tauri macro library. Setting Cargo `profile.release.build-override.strip="none"` for this build resolved the issue. No system security setting, deployed binary, or third-party source was patched to work around it. The final application uses a local ad-hoc development signature, not Developer ID notarization.

Frontend generation/type checking/production build passed. Release library tests with the final deployment target 14.2, custom-protocol feature and build-override setting passed: 271 passed, 0 failed, 3 existing opt-in real-model tests ignored. The packaged app passed `codesign --verify --deep --strict`.

On 2026-11-09 the installed application was switched to HY-MT2 through its UI. A restart at 14:36:11 UTC verified the saved HY-MT2 tier, Russian to simplified Chinese, Nemotron ASR, and disabled analytics. Five silent tests reused existing ASR text without playback, capture, or ASR. Cold text-to-event/render-ack latency was 2664/2702 ms; four warm tests were 212–325/239–356 ms. The Chinese text was visible in the main window, and the real floating frontend acknowledged frames. Independent visual confirmation of the floating panel remains pending. These are not speech end-to-end measurements. Detailed local evidence is in the parent workspace silent-overlay-verification.json and silent-overlay-evidence.log.

Store real model assets and the private runtime in the application data folder rather than symlinking into Documents. On this machine, app model discovery stalled opening the Documents-linked config even though terminal reads worked. APFS-cloning the verified model and runtime into application data resolved startup without granting broader file access. Original models and the original application are preserved.

The model license is Apache-2.0. A display-label correction has passed the same 271 release tests and packaging; its final deployment is pending manual Mac unlock. No security settings were changed.
