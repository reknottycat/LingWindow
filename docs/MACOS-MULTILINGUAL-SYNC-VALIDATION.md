# macOS multilingual sync validation

Validated on M6 (Apple Silicon), 2026-11-12 UTC. Source baseline: `e770cdb0fe8a4b7152e02aae29b68f69a848e9b1`.

## Scope

Reused the original clean macOS build workspace. Shared multilingual language selectors, HY-MT2 labels, model-status refresh, and overlay double-click restore behavior are included without a separate Mac implementation. Source choices remain constrained by the selected ASR; selectable language pairs are not a claim of tested speech quality. Existing models and preferences were not changed.

## Verified

- TypeScript typecheck passed.
- Overlay regression tests: 9 passed, including double-click restore/drag separation. These are source/render tests, not Mac desktop gesture verification.
- HY-MT2 worker input tests: 4 passed.
- Release app built successfully using existing Rust 1.90.0 and the privacy-preserving build wrapper.
- Ad-hoc code signature passed `codesign --verify --deep --strict`. No Apple notarization was performed.
- Candidate executable SHA-256: `483aea2a39712291704c6c3a05e33cb120a8fe6819f0eecf1193dfb2615252a6`.

## Actual local model inference

Used the existing Mac MLX HY-MT2 model, MLX 0.32.4 and mlx-lm 0.32.2, with the updated worker. Three synthetic text requests returned successfully:

| Direction | Input | Output |
| --- | --- | --- |
| en → zh | The system translates locally without sending audio to a server. | 该系统在本地进行翻译，无需将音频发送到服务器。 |
| zh → en | 请明天下午三点到办公室开会。 | Please attend a meeting in the office at 3 PM tomorrow. |
| ru → en | Пожалуйста, отправьте документы завтра утром. | Please send the documents tomorrow morning. |

The runtime emitted a warning about unrecognized dynamic RoPE configuration keys but completed all three requests. This is not a benchmark or a full language-pair quality assessment. No microphone capture or playback was performed.

## Installation boundary

Both installed AudioScript and LingWindow were already running. They were not terminated, replaced, or reconfigured. The candidate is built, not installed; the running Mac app remains the prior version. GUI language selection and double-click restore still need verification after a safe application switch.
