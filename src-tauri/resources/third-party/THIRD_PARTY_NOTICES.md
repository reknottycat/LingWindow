# Third-party notices for LingWindow

LingWindow is an independent derivative of AudioScript v0.7.0 by Nico (niyongsheng). The original MIT copyright is retained in LICENSE and AudioScript-MIT.txt. No endorsement by the original authors is implied.

## Models actually bundled

- **Zipformer streaming small bilingual zh/en, 2023-02-16**: k2-fsa / sherpa-onnx, Apache License 2.0. Source: https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models . Distributed files are encoder and joiner int8 ONNX, fp32 decoder, tokenizer and token table from the upstream archive. This derivative reuses AudioScript's bundled files without re-training or changing weights. https://github.com/k2-fsa/sherpa-onnx
- **Silero VAD**: Copyright (c) 2020-present Silero Team, MIT. Source: https://github.com/snakers4/silero-vad . The ONNX voice-activity model is included without further modification by this derivative. License: Silero-VAD-MIT.txt.
- **Helsinki-NLP/opus-mt-en-zh**: Helsinki-NLP / OPUS-MT, Apache License 2.0. Model card and source: https://huggingface.co/Helsinki-NLP/opus-mt-en-zh . AudioScript's model preparation converts the original Marian model to CTranslate2 int8 and exports tokenizer/vocabulary JSON. This derivative reuses those converted files. See OPUS-en-zh-model-card.md.
- **Helsinki-NLP/opus-mt-zh-en**: Helsinki-NLP / OPUS-MT, Creative Commons Attribution 4.0 International. Model card and original source: https://huggingface.co/Helsinki-NLP/opus-mt-zh-en . License: https://creativecommons.org/licenses/by/4.0/ and CC-BY-4.0.txt. Modification: AudioScript's preparation converts the original Marian model to CTranslate2 int8 and exports tokenizer/vocabulary JSON; LingWindow reuses those converted files. Credit is retained to Helsinki-NLP / OPUS-MT. No endorsement is implied. See OPUS-zh-en-model-card.md for original attribution and citations.

The application About page also lists model licenses and credits. License texts and original model cards are bundled under Contents/Resources/third-party on macOS.

## Native engines

- sherpa-onnx (k2-fsa), Apache-2.0: https://github.com/k2-fsa/sherpa-onnx
- CTranslate2 (OpenNMT), MIT: https://github.com/OpenNMT/CTranslate2 . See CTranslate2-MIT.txt.
- ONNX Runtime (Microsoft), MIT: https://github.com/microsoft/onnxruntime . See ONNX-Runtime-MIT.txt.
- Apple system frameworks are provided by macOS, not redistributed as third-party libraries in this bundle.

The bundled-model directory does **not** include HY-MT2, Nemotron, NLLB, or a Python environment. Optional models are separate downloads/installations and retain their own licenses. In particular NLLB is CC-BY-NC-4.0 and restricted to non-commercial use. HY-MT2 installation and its Apache-2.0 attribution are documented separately.

## Download source transparency

Nemotron's optional archive uses a domestic ModelScope mirror with the same pinned SHA-256 and size as the upstream release. The existing local model is reused during migration; it is not downloaded again or embedded in the application. Other models retain their original sources where a verified identical domestic artifact is not configured.
