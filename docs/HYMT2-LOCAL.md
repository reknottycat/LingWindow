# HY-MT2 local translation in LingWindow

This local adaptation adds HY-MT2 1.8B MLX 4-bit as the default translation tier on Apple Silicon Macs. Existing saved model selections are preserved. OPUS-MT, NLLB and LMT remain available. LingWindow is an independently maintained derivative of AudioScript v0.7.0, not an official AudioScript release.

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

For the full app build, run frontend type checking/build and Rust tests using the repository's normal toolchain. Release localization covers all supported interface languages. Do not represent text-only verification as an audio end-to-end test.

## Reproducible M6 build notes

The verified local toolchain is Rust/Cargo 1.90.0, CMake 4.4.4, pnpm 10.29.2 and typeshare-cli 1.13.4. Rust dependencies were obtained through the rsproxy.cn mirror with the original Cargo.lock checksums retained. Built-in model assets were copied from the original installed application, not downloaded again. The sherpa-onnx 1.13.8 arm64 static library was obtained from the k2-fsa official GitHub release and verified against its release SHA-256 `9091bf160dc7fdacedbc906b212badf53c2993f4e5277a0e03998e96c31d60da`.

The current macOS 27.2 linker produced an invalid LINKEDIT string-pool alignment when stripping a build-time Tauri macro library. Setting Cargo `profile.release.build-override.strip="none"` for this build resolved the issue. No system security setting, deployed binary, or third-party source was patched to work around it. The final application uses a local ad-hoc development signature, not Developer ID notarization.

Model assets and the private runtime belong in application data, not in the Git repository. Do not use a Documents symlink as the deployed model location: sandbox and privacy access can differ between terminal and packaged app. Preserve existing data during migration and verify cloned model hashes before use.

See the release notes for checks actually run on a particular build. Text-only verification is not speech end-to-end verification. The application is locally ad-hoc signed, not Developer ID notarized.
