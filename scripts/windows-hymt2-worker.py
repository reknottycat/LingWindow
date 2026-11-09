"""Offline JSONL HY-MT2 worker. Stdout is reserved for protocol messages."""
import contextlib
import json
import os
from pathlib import Path
import sys
import time

MAX_CHARS = 2000
MAX_NEW_TOKENS = 512
MAX_LINE_BYTES = 32768
LANGUAGES = dict(zip(
    'zh en fr pt es ja tr ru ar ko th it de vi ms id tl hi zh-Hant pl cs nl km my fa gu ur te mr he bn ta uk bo kk mn ug yue'.split(),
    ['Chinese', 'English', 'French', 'Portuguese', 'Spanish', 'Japanese', 'Turkish',
     'Russian', 'Arabic', 'Korean', 'Thai', 'Italian', 'German', 'Vietnamese',
     'Malay', 'Indonesian', 'Filipino', 'Hindi', 'Traditional Chinese', 'Polish',
     'Czech', 'Dutch', 'Khmer', 'Burmese', 'Persian', 'Gujarati', 'Urdu', 'Telugu',
     'Marathi', 'Hebrew', 'Bengali', 'Tamil', 'Ukrainian', 'Tibetan', 'Kazakh',
     'Mongolian', 'Uyghur', 'Cantonese']))


def validate_request(request):
    if not isinstance(request, dict):
        raise ValueError('请求必须是 JSON 对象')
    if request.get('op') != 'translate':
        raise ValueError('不支持的操作')
    if type(request.get('id')) is not int or request['id'] < 0:
        raise ValueError('请求 id 必须是非负整数')
    source, target, text = (request.get(k) for k in ('source', 'target', 'text'))
    if source not in LANGUAGES or target not in LANGUAGES:
        raise ValueError('不支持的语言代码')
    if not isinstance(text, str) or not text.strip() or '\0' in text:
        raise ValueError('翻译文本不能为空')
    if len(text) > MAX_CHARS:
        raise ValueError('翻译文本超过 2000 字符限制，请缩短字幕分句')
    return source, target, text


def make_prompt(target, text):
    return (f'Translate the following text into {LANGUAGES[target]}. '
            'Note that you should only output the translated result without any additional explanation:\n\n'
            + text)


def select_device(torch, allow_cpu):
    if torch.cuda.is_available():
        dtype = torch.bfloat16 if torch.cuda.is_bf16_supported() else torch.float16
        return 'cuda', dtype
    if not allow_cpu:
        raise RuntimeError('CUDA 不可用；请检查 NVIDIA 驱动和 CUDA 版 PyTorch。CPU 模式需显式启用，不保证实时性能')
    cpu_bf16 = getattr(getattr(torch, 'cpu', None), '_is_avx512_bf16_supported', lambda: False)
    return 'cpu', torch.bfloat16 if cpu_bf16() else torch.float32


def check_completion(ids, eos_ids):
    eos = set(eos_ids if isinstance(eos_ids, (list, tuple)) else [eos_ids])
    if not ids or ids[-1] not in eos:
        raise RuntimeError('翻译未正常结束或达到 512 token 上限，已拒绝截断结果；请缩短字幕分句')


class Translator:
    def __init__(self, directory):
        for key in ('HF_HUB_OFFLINE', 'TRANSFORMERS_OFFLINE', 'HF_DATASETS_OFFLINE'):
            os.environ[key] = '1'
        os.environ['HF_HUB_DISABLE_TELEMETRY'] = '1'
        import torch
        from transformers import AutoModelForCausalLM, AutoTokenizer
        from transformers.models.auto.configuration_auto import CONFIG_MAPPING
        if 'hunyuan_v1_dense' not in CONFIG_MAPPING:
            raise RuntimeError('需要支持 hunyuan_v1_dense 的 transformers>=5.6.0')
        directory = Path(directory).resolve(strict=True)
        config = json.loads((directory / 'config.json').read_text(encoding='utf-8'))
        if config.get('model_type') != 'hunyuan_v1_dense' or config.get('architectures') != ['HunYuanDenseV1ForCausalLM']:
            raise RuntimeError('模型不是官方 Hy-MT2-1.8B dense 架构；不支持 MLX 模型')
        runtime_path = directory / 'runtime.json'
        runtime = json.loads(runtime_path.read_text(encoding='utf-8')) if runtime_path.exists() else {}
        self.device, dtype = select_device(torch, runtime.get('allow_cpu') is True)
        self.torch = torch
        self.dtype = str(dtype).removeprefix('torch.')
        self.tokenizer = AutoTokenizer.from_pretrained(directory, local_files_only=True, trust_remote_code=False)
        self.model = AutoModelForCausalLM.from_pretrained(
            directory, local_files_only=True, trust_remote_code=False,
            use_safetensors=True, dtype=dtype, attn_implementation='sdpa').to(self.device).eval()
        self.eos = self.model.generation_config.eos_token_id
        if self.eos is None:
            self.eos = self.tokenizer.eos_token_id
        if self.eos is None:
            raise RuntimeError('模型缺少结束 token')

    def translate(self, source, target, text):
        inputs = self.tokenizer.apply_chat_template(
            [{'role': 'user', 'content': make_prompt(target, text)}],
            add_generation_prompt=True, tokenize=True, return_dict=True, return_tensors='pt')
        inputs = {key: value.to(self.device) for key, value in inputs.items()}
        length = inputs['input_ids'].shape[-1]
        if length + MAX_NEW_TOKENS > self.model.config.max_position_embeddings:
            raise ValueError('输入超出模型上下文限制')
        with self.torch.inference_mode():
            output = self.model.generate(**inputs, max_new_tokens=MAX_NEW_TOKENS,
                                         do_sample=False, repetition_penalty=1.05,
                                         eos_token_id=self.eos, pad_token_id=self.tokenizer.pad_token_id)
        tokens = output[0][length:].tolist()
        check_completion(tokens, self.eos)
        result = self.tokenizer.decode(tokens, skip_special_tokens=True).strip()
        if not result:
            raise RuntimeError('模型返回空译文')
        return result


def emit(stream, message):
    stream.write(json.dumps(message, ensure_ascii=False) + '\n')
    stream.flush()


def serve(translator, source, output):
    while True:
        line = source.readline(MAX_LINE_BYTES + 1)
        if not line:
            return
        request_id = None
        try:
            if len(line.encode('utf-8')) > MAX_LINE_BYTES:
                while not line.endswith('\n'):
                    line = source.readline(MAX_LINE_BYTES + 1)
                    if not line:
                        break
                raise ValueError('请求行过长')
            request = json.loads(line)
            if isinstance(request, dict):
                request_id = request.get('id')
                if request.get('op') == 'shutdown':
                    return
            source_lang, target, text = validate_request(request)
            with contextlib.redirect_stdout(sys.stderr):
                result = translator.translate(source_lang, target, text)
            emit(output, {'type': 'translation', 'id': request_id, 'text': result})
        except Exception as error:
            emit(output, {'type': 'error', 'id': request_id, 'error': str(error)})


def main():
    for stream in (sys.stdin, sys.stdout, sys.stderr):
        if hasattr(stream, 'reconfigure'):
            stream.reconfigure(encoding='utf-8', errors='strict')
    output = sys.stdout
    try:
        if len(sys.argv) != 2:
            raise ValueError('用法：python -I -u windows-hymt2-worker.py MODEL_DIR')
        started = time.monotonic()
        with contextlib.redirect_stdout(sys.stderr):
            translator = Translator(sys.argv[1])
            warmup_started = time.monotonic()
            translator.translate('en', 'zh', 'Hello.')
        warmup_ms = round((time.monotonic() - warmup_started) * 1000)
        emit(output, {'type': 'ready', 'protocol': 1, 'backend': 'transformers',
                      'model': 'Hy-MT2-1.8B', 'device': translator.device, 'dtype': translator.dtype,
                      'torch': translator.torch.__version__, 'warmup_ms': warmup_ms,
                      'startup_ms': round((time.monotonic() - started) * 1000),
                      'warning': 'CPU 模式不保证实时性能' if translator.device == 'cpu' else None})
        serve(translator, sys.stdin, output)
        return 0
    except Exception as error:
        emit(output, {'type': 'error', 'id': None, 'error': str(error)})
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
