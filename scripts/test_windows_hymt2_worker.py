import importlib.util
import io
import json
from pathlib import Path
import unittest


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


worker = load('worker', 'windows-hymt2-worker.py')
setup = load('setup_hymt2', 'setup-windows-hymt2.py')


class FakeTranslator:
    def translate(self, source, target, text):
        if text == 'fail':
            raise RuntimeError('测试推理失败')
        print('测试第三方日志')
        return '测试译文'


class WorkerTests(unittest.TestCase):
    def request(self, **changes):
        result = dict(op='translate', id=1, source='ru', target='zh', text='Привет')
        result.update(changes)
        return result

    def run_lines(self, lines):
        out = io.StringIO()
        worker.serve(FakeTranslator(), io.StringIO('\n'.join(lines) + '\n'), out)
        return [json.loads(line) for line in out.getvalue().splitlines()]

    def test_request(self):
        self.assertEqual(worker.validate_request(self.request()), ('ru', 'zh', 'Привет'))

    def test_limits(self):
        worker.validate_request(self.request(text='字' * 2000))
        for changes in [dict(text='字' * 2001), dict(text='  '), dict(text=None), dict(text='a\0b'),
                        dict(source='auto'), dict(target='invalid'), dict(id=True),
                        dict(id=-1), dict(op='unknown')]:
            with self.assertRaises(ValueError):
                worker.validate_request(self.request(**changes))

    def test_prompt(self):
        self.assertIn('Chinese', worker.make_prompt('zh', 'Привет'))
        self.assertTrue(worker.make_prompt('zh', 'Привет').endswith('Привет'))

    def test_eos_and_truncation(self):
        worker.check_completion([1, 2, 120020], 120020)
        worker.check_completion([1] * 511 + [120020], [120020, 2])
        for ids in [[], [1] * 512, [1, 2]]:
            with self.assertRaises(RuntimeError):
                worker.check_completion(ids, 120020)

    def test_protocol_and_shutdown(self):
        result = self.run_lines([json.dumps(self.request()), '{"op":"shutdown"}', json.dumps(self.request())])
        self.assertEqual(result, [{'type': 'translation', 'id': 1, 'text': '测试译文'}])

    def test_malformed_continues(self):
        result = self.run_lines(['invalid', '[]', json.dumps(self.request(text='fail')), json.dumps(self.request(id=4))])
        self.assertEqual([r['type'] for r in result], ['error', 'error', 'error', 'translation'])
        self.assertEqual(result[2]['id'], 1)
        self.assertEqual(result[3]['id'], 4)

    def test_long_line_drained(self):
        result = self.run_lines([' ' * 40000, json.dumps(self.request())])
        self.assertEqual([r['type'] for r in result], ['error', 'translation'])

    def test_cuda_cpu_policy(self):
        class CUDA:
            available = True
            bf16 = True
            def is_available(self): return self.available
            def is_bf16_supported(self): return self.bf16
        class Torch:
            cuda = CUDA()
            bfloat16, float16, float32 = 'bf16', 'fp16', 'fp32'
        torch = Torch()
        self.assertEqual(worker.select_device(torch, False), ('cuda', 'bf16'))
        torch.cuda.bf16 = False
        self.assertEqual(worker.select_device(torch, False), ('cuda', 'fp16'))
        torch.cuda.available = False
        with self.assertRaises(RuntimeError): worker.select_device(torch, False)
        self.assertEqual(worker.select_device(torch, True), ('cpu', 'fp32'))
        from types import SimpleNamespace
        torch.cpu = SimpleNamespace(_is_avx512_bf16_supported=lambda: True)
        self.assertEqual(worker.select_device(torch, True), ('cpu', 'bf16'))

    def test_embedded_launch_failure_is_json(self):
        import subprocess
        import sys
        script = Path(__file__).with_name('windows-hymt2-worker.py').read_text(encoding='utf-8')
        result = subprocess.run([sys.executable, '-I', '-X', 'utf8', '-u', '-c', script],
                                capture_output=True, text=True, encoding='utf-8', timeout=10)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(json.loads(result.stdout)['type'], 'error')
        self.assertEqual(result.stderr, '')

    def test_ready_after_warmup(self):
        from types import SimpleNamespace
        from unittest.mock import patch, Mock
        translator = SimpleNamespace(device='cpu', dtype='bfloat16',
                                     torch=SimpleNamespace(__version__='test', get_num_threads=lambda: 4),
                                     translate=Mock(return_value='你好。'))
        out = io.StringIO()
        with patch.object(worker, 'Translator', return_value=translator), \
             patch.object(worker.sys, 'argv', ['-c', '中文 模型']), \
             patch.object(worker.sys, 'stdin', io.StringIO('{"op":"shutdown"}\n')), \
             patch.object(worker.sys, 'stdout', out):
            self.assertEqual(worker.main(), 0)
        translator.translate.assert_called_once_with('en', 'zh', 'Hello.')
        ready = json.loads(out.getvalue())
        self.assertEqual(ready['type'], 'ready')
        self.assertEqual(ready['threads'], 4)

    def test_failed_warmup_never_ready(self):
        from types import SimpleNamespace
        from unittest.mock import patch, Mock
        translator = SimpleNamespace(translate=Mock(side_effect=RuntimeError('预热失败')))
        out = io.StringIO()
        with patch.object(worker, 'Translator', return_value=translator), \
             patch.object(worker.sys, 'argv', ['-c', '中文 模型']), \
             patch.object(worker.sys, 'stdout', out):
            self.assertEqual(worker.main(), 1)
        self.assertEqual(json.loads(out.getvalue())['type'], 'error')

    def test_manifest(self):
        self.assertEqual(setup.MANIFEST['model.safetensors']['size'], 4077072784)
        self.assertTrue(all(len(item['sha256']) == 64 for item in setup.MANIFEST.values()))
        self.assertTrue(all(len(item['revision']) == 40 for item in setup.MANIFEST.values()))

    def test_range_resume(self):
        import tempfile
        from unittest.mock import patch
        class Response(io.BytesIO):
            status = 206
            headers = {'Content-Range': 'bytes 2-5/6'}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'model.partial'
            path.write_bytes(b'ab')
            with patch.object(setup.urllib.request, 'urlopen', return_value=Response(b'cdef')) as request:
                setup.download_large('https://modelscope.cn/test', path, 6)
                self.assertEqual(request.call_args.args[0].headers['Range'], 'bytes=2-5')
            self.assertEqual(path.read_bytes(), b'abcdef')

    def test_hash_validation(self):
        import hashlib
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'test'
            path.write_bytes(b'verified')
            entry = dict(size=8, sha256=hashlib.sha256(b'verified').hexdigest())
            self.assertTrue(setup.verified(path, entry))
            path.write_bytes(b'modified')
            self.assertFalse(setup.verified(path, entry))

    def test_explicit_copy_preserves_old_runtime_and_only_copies_verified_models(self):
        import hashlib
        import tempfile
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as directory:
            old, new = Path(directory) / '旧模型', Path(directory) / '新模型'
            old.mkdir()
            (old / 'model.safetensors').write_bytes(b'fixture')
            (old / 'runtime.json').write_text('{"python":"old"}', encoding='utf-8')
            manifest = {'model.safetensors': dict(size=7, sha256=hashlib.sha256(b'fixture').hexdigest())}
            with patch.object(setup, 'MANIFEST', manifest):
                setup.copy_verified_model(old, new)
            self.assertEqual((new / 'model.safetensors').read_bytes(), b'fixture')
            self.assertEqual((old / 'model.safetensors').read_bytes(), b'fixture')
            self.assertTrue((old / 'runtime.json').is_file())
            self.assertFalse((new / 'runtime.json').exists())

    def test_explicit_copy_rejects_unverified_or_same_directory_before_writing(self):
        import hashlib
        import tempfile
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as directory:
            old, new = Path(directory) / 'old', Path(directory) / 'new'
            old.mkdir()
            (old / 'model.safetensors').write_bytes(b'changed')
            manifest = {'model.safetensors': dict(size=7, sha256=hashlib.sha256(b'fixture').hexdigest())}
            with patch.object(setup, 'MANIFEST', manifest):
                with self.assertRaises(RuntimeError):
                    setup.copy_verified_model(old, new)
                with self.assertRaises(ValueError):
                    setup.copy_verified_model(old, old)
            self.assertFalse(new.exists())


if __name__ == '__main__':
    unittest.main()
