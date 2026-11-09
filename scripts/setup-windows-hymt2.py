"""Install a project-local Python runtime and hash-verified official ModelScope model."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import urllib.parse
import urllib.request
import venv

MODEL_ID = 'Tencent-Hunyuan/Hy-MT2-1.8B'
# Pinned official ModelScope file revisions, sizes and SHA-256 (2026-11-09).
MANIFEST = {'LICENSE.txt': {'size': 11629,
                 'sha256': '1af3c6dc0c697277cbb6b68720787c1caa43a79c5626bf9f19cd8c00de9c8cd4',
                 'revision': '09fd81b4d8952bad2906e5c72f167bc84b0c6921'},
 'README.md': {'size': 15255,
               'sha256': '6820063a1296c282cae052c59041c36b4100371ece01dc6738ab8bfe4828a455',
               'revision': '09fd81b4d8952bad2906e5c72f167bc84b0c6921'},
 'chat_template.jinja': {'size': 654,
                         'sha256': 'b7491ec0e9c869dfce20f2176758099bf248d979dd05530ede99deb21698acee',
                         'revision': '136293b8886aff2c0dca64e953e26346a1fc2fbd'},
 'config.json': {'size': 1348,
                 'sha256': 'da40c514cc74a5748a2e591b1b95fca4b7e94de05349abe4ea4164a82641de1a',
                 'revision': '136293b8886aff2c0dca64e953e26346a1fc2fbd'},
 'generation_config.json': {'size': 221,
                            'sha256': '0e28667f1cb4c7b880b9223b2d87978f88e79ed7ae037de1021f826c18d4ed6f',
                            'revision': '136293b8886aff2c0dca64e953e26346a1fc2fbd'},
 'model.safetensors': {'size': 4077072784,
                       'sha256': '29e9117a44c79f81857613601968ff482d8a23c2d6736a1710bba9e5ca4762e5',
                       'revision': '136293b8886aff2c0dca64e953e26346a1fc2fbd'},
 'special_tokens_map.json': {'size': 488,
                             'sha256': 'bb9f59990034dae326581b9c62471523975417869f78a244b7ae2ce8cbb085eb',
                             'revision': '136293b8886aff2c0dca64e953e26346a1fc2fbd'},
 'tokenizer.json': {'size': 9527287,
                    'sha256': 'b475bbef1b0b2fd57dcb865332b546475bd1ede2deb3bb91bafd0c047a8a530a',
                    'revision': '136293b8886aff2c0dca64e953e26346a1fc2fbd'},
 'tokenizer_config.json': {'size': 165815,
                           'sha256': '53bd8581b601a8ee9caefeb988207de50b3fc0b733295bdf5ad68dec4cc0b07c',
                           'revision': '136293b8886aff2c0dca64e953e26346a1fc2fbd'}}


def digest(path):
    sha = hashlib.sha256()
    with path.open('rb') as handle:
        for chunk in iter(lambda: handle.read(8 * 1024 * 1024), b''):
            sha.update(chunk)
    return sha.hexdigest()


def verified(path, entry):
    return path.is_file() and path.stat().st_size == entry['size'] and digest(path) == entry['sha256']


def download_large(url, partial, total):
    # Resume only at verified HTTP range boundaries; final SHA-256 remains mandatory.
    offset = partial.stat().st_size if partial.exists() else 0
    if offset > total:
        partial.unlink()
        offset = 0
    with partial.open('ab') as output:
        while offset < total:
            end = min(total, offset + 8 * 1024 * 1024) - 1
            for attempt in range(3):
                try:
                    request = urllib.request.Request(url, headers={'Range': f'bytes={offset}-{end}'})
                    with urllib.request.urlopen(request, timeout=120) as response:
                        expected = f'bytes {offset}-{end}/{total}'
                        if response.status != 206 or response.headers.get('Content-Range') != expected:
                            raise RuntimeError('服务器未返回所请求的文件范围')
                        data = response.read(end - offset + 2)
                    if len(data) != end - offset + 1:
                        raise RuntimeError('下载范围大小不符')
                    break
                except Exception:
                    if attempt == 2:
                        raise
                    time.sleep(2 ** attempt)
            output.write(data)
            output.flush()
            offset = end + 1
            print(f'模型下载进度：{offset}/{total}', flush=True)


def download_model(directory):
    directory.mkdir(parents=True, exist_ok=True)
    for name, entry in MANIFEST.items():
        target = directory / name
        if verified(target, entry):
            print(f'校验通过：{name}', flush=True)
            continue
        url = ('https://modelscope.cn/api/v1/models/' + MODEL_ID + '/repo?' +
               urllib.parse.urlencode({'Revision': entry['revision'], 'FilePath': name}))
        partial = target.with_suffix(target.suffix + '.partial')
        print(f'下载官方模型：{name} ({entry["size"]} bytes)', flush=True)
        if entry['size'] > 64 * 1024 * 1024:
            download_large(url, partial, entry['size'])
        else:
            with urllib.request.urlopen(url, timeout=120) as response, partial.open('wb') as output:
                while chunk := response.read(8 * 1024 * 1024):
                    output.write(chunk)
        if not verified(partial, entry):
            partial.unlink(missing_ok=True)
            raise RuntimeError(f'SHA256 或大小校验失败：{name}，未启用该文件')
        os.replace(partial, target)
    (directory / 'download-manifest.json').write_text(
        json.dumps({'source': MODEL_ID, 'files': MANIFEST}, indent=2), encoding='utf-8')


def main():
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, 'reconfigure'):
            stream.reconfigure(encoding='utf-8')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model-dir', type=Path, required=True,
                        help='应用模型根目录中的 mt-hymt2 文件夹（不是根目录）')
    parser.add_argument('--venv-dir', type=Path, help='默认 MODEL_DIR/.venv，不修改全局 Python')
    parser.add_argument('--cpu', action='store_true', help='显式启用 CPU，不保证实时性能')
    parser.add_argument('--download-only', action='store_true', help='只下载并校验模型，不创建 runtime.json')
    args = parser.parse_args()
    if sys.version_info < (3, 10):
        parser.error('需要 Python 3.10 或更新版本，建议 3.12')
    directory = args.model_dir.expanduser().resolve()
    download_model(directory)
    if args.download_only:
        return
    envdir = (args.venv_dir or directory / '.venv').resolve()
    python = envdir / ('Scripts/python.exe' if os.name == 'nt' else 'bin/python')
    if not python.exists():
        venv.EnvBuilder(with_pip=True).create(envdir)
    subprocess.run([str(python), '-X', 'utf8', '-m', 'pip', 'install', '--upgrade', 'pip'], check=True)
    variant = 'cpu' if args.cpu else 'cu128'
    index = 'https://download.pytorch.org/whl/' + variant
    subprocess.run([str(python), '-X', 'utf8', '-m', 'pip', 'install', 'torch==2.11.0+' + variant,
                    '--index-url', index], check=True)
    requirements = Path(__file__).with_name('requirements-windows-hymt2.txt')
    subprocess.run([str(python), '-X', 'utf8', '-m', 'pip', 'install', '-r', str(requirements)], check=True)
    check = ('import torch; from transformers.models.auto.configuration_auto import CONFIG_MAPPING; '
             'assert "hunyuan_v1_dense" in CONFIG_MAPPING; '
             + ('' if args.cpu else 'assert torch.cuda.is_available(), "CUDA 不可用，请检查驱动；CPU 需 --cpu"; ')
             + 'print(torch.__version__, torch.cuda.is_available())')
    subprocess.run([str(python), '-I', '-X', 'utf8', '-c', check], check=True)
    temporary = directory / 'runtime.json.partial'
    temporary.write_text(json.dumps({'python': str(python), 'allow_cpu': args.cpu}, indent=2), encoding='utf-8')
    os.replace(temporary, directory / 'runtime.json')
    print('安装及完整性校验完成。CPU 模式不保证实时性能。' if args.cpu else 'CUDA 运行时安装及完整性校验完成。')


if __name__ == '__main__':
    main()
