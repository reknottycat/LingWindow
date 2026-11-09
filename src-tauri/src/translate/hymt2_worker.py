"""Private JSON-lines worker. No audio, HTTP server, downloads, or remote code."""
import contextlib
import importlib.metadata
import json
import os
from pathlib import Path
import sys

PROTOCOL = sys.stdout
MAX_CHARS = 2000
PROMPT = "将以下文本翻译为中文，注意只需要输出翻译后的结果，不要额外解释：\n"


def reply(value):
    PROTOCOL.write(json.dumps(value, ensure_ascii=False) + "\n")
    PROTOCOL.flush()


def validate_request(request):
    if not isinstance(request, dict) or request.get("op") != "translate":
        raise ValueError("invalid operation")
    text = request.get("text")
    if not isinstance(text, str) or not text.strip() or len(text) > MAX_CHARS:
        raise ValueError("text must contain 1–2000 characters")
    if "\x00" in text:
        raise ValueError("NUL is not allowed")
    request_id = request.get("id")
    if isinstance(request_id, bool) or not isinstance(request_id, int) or request_id < 1:
        raise ValueError("invalid request id")
    if request.get("source") not in ("en", "ru") or request.get("target") != "zh":
        raise ValueError("supported directions: en/ru → zh")
    return text.strip()


def main():
    for key in ("HF_HUB_OFFLINE", "TRANSFORMERS_OFFLINE", "HF_DATASETS_OFFLINE"):
        os.environ[key] = "1"
    os.environ["HF_HUB_DISABLE_TELEMETRY"] = "1"
    os.environ["TOKENIZERS_PARALLELISM"] = "false"
    model_dir = Path(sys.argv[1])
    if not model_dir.is_absolute() or not model_dir.is_dir():
        raise ValueError("model path must be an existing absolute directory")
    with contextlib.redirect_stdout(sys.stderr):
        import mlx.core as mx
        from mlx_lm import load, stream_generate
        from mlx_lm.sample_utils import make_sampler, make_logits_processors
        model, tokenizer = load(str(model_dir), tokenizer_config={"trust_remote_code": False, "local_files_only": True})
        sampler = make_sampler(temp=0.7, top_p=0.6, top_k=20)
        processors = make_logits_processors(repetition_penalty=1.05, repetition_context_size=262144)

        def translate(text, request_id, max_tokens=512):
            mx.random.seed((42 + request_id) % (2 ** 32))
            prompt = tokenizer.apply_chat_template(
                [{"role": "user", "content": PROMPT + text}],
                tokenize=False, add_generation_prompt=True,
            )
            chunks = []
            last = None
            for last in stream_generate(model, tokenizer, prompt=prompt, max_tokens=max_tokens,
                                        sampler=sampler, logits_processors=processors):
                chunks.append(last.text)
            if last is None or last.finish_reason != "stop":
                raise ValueError("translation reached the output limit; use a shorter text")
            result = "".join(chunks).strip()
            if not result:
                raise ValueError("empty translation")
            return result

        translate("Hello.", 0, 32)
    reply({"type": "ready", "protocol": 1,
           "mlx": importlib.metadata.version("mlx"),
           "mlx_lm": importlib.metadata.version("mlx-lm")})
    for line in sys.stdin:
        request_id = None
        try:
            if len(line) > 32768:
                raise ValueError("request too large")
            request = json.loads(line)
            if isinstance(request, dict) and request.get("op") == "shutdown":
                return
            request_id = request.get("id") if isinstance(request, dict) else None
            text = validate_request(request)
            with contextlib.redirect_stdout(sys.stderr):
                result = translate(text, request_id)
            reply({"type": "translation", "id": request_id, "text": result})
        except Exception as exc:
            reply({"type": "error", "id": request_id, "error": str(exc)[:2000]})


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        reply({"type": "error", "error": str(exc)[:2000]})
        sys.exit(1)
