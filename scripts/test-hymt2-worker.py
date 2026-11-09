#!/usr/bin/env python3
"""Run input tests; --model also exercises the real local worker without audio."""
import argparse
import importlib.util
import json
from pathlib import Path
import selectors
import subprocess
import sys
import time
import unittest

sys.dont_write_bytecode = True
WORKER = Path(__file__).resolve().parents[1] / "src-tauri/src/translate/hymt2_worker.py"
spec = importlib.util.spec_from_file_location("hymt2_worker", WORKER)
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)


def request(text="Hello", source="en", target="zh", request_id=1):
    return {"op": "translate", "id": request_id, "source": source, "target": target, "text": text}


class InputTests(unittest.TestCase):
    def test_unicode_and_newlines(self):
        self.assertEqual(worker.validate_request(request("  Привет 🌏\n世界  ", "ru")), "Привет 🌏\n世界")

    def test_limits(self):
        self.assertEqual(len(worker.validate_request(request("界" * 2000))), 2000)
        for text in ("", " \n ", "界" * 2001, "a\0b", None):
            with self.assertRaises(ValueError):
                worker.validate_request(request(text))

    def test_language_scope(self):
        for source, target in (("fr", "zh"), ("zh", "en"), ("ru", "en")):
            with self.assertRaises(ValueError):
                worker.validate_request(request(source=source, target=target))

    def test_invalid_ids_and_operations(self):
        for value in (None, [], {"op": "run"}, request(request_id=True), request(request_id=-1)):
            with self.assertRaises(ValueError):
                worker.validate_request(value)


def smoke(model):
    started = time.monotonic()
    process = subprocess.Popen([sys.executable, "-I", "-u", "-c", WORKER.read_text(), str(model.resolve())],
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    events = []

    def receive(timeout=90):
        with selectors.DefaultSelector() as selector:
            selector.register(process.stdout, selectors.EVENT_READ)
            if not selector.select(timeout):
                raise TimeoutError("本地 worker 响应超时")
        line = process.stdout.readline()
        if not line:
            raise RuntimeError("本地 worker 意外退出")
        return json.loads(line)

    def send(value):
        t0 = time.monotonic()
        process.stdin.write(json.dumps(value, ensure_ascii=False) + "\n")
        process.stdin.flush()
        reply = receive()
        events.append({"request": value, "response": reply, "elapsedMs": round((time.monotonic() - t0) * 1000, 2)})
        return reply

    try:
        ready = receive()
        events.append({"ready": ready, "elapsedMs": round((time.monotonic() - started) * 1000, 2)})
        assert ready.get("type") == "ready", ready
        for i, (source, text) in enumerate([
            ("en", "Hello. The meeting will begin in ten minutes."),
            ("ru", "Здравствуйте. Встреча начнётся через десять минут."),
            ("en", "The system translates locally without sending audio to a server."),
        ], 1):
            result = send(request(text, source, request_id=i))
            assert result.get("type") == "translation" and result.get("id") == i, result
            assert any("\u4e00" <= char <= "\u9fff" for char in result["text"]), result
        invalid = send(request("Bonjour", "fr", request_id=4))
        assert invalid.get("type") == "error", invalid
        # A rejected request must not poison a later valid request.
        recovered = send(request("Thank you.", request_id=5))
        assert recovered.get("type") == "translation", recovered
        repeated = send(request("Thank you.", request_id=5))
        assert repeated["text"] == recovered["text"], "固定请求种子应可重复"
        process.stdin.write('{"op":"shutdown"}\n')
        process.stdin.flush()
        assert process.wait(timeout=10) == 0
        events.append({"shutdownExitCode": process.returncode})
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
    return {"scope": "silent text-to-MT protocol only; not overlay or audio E2E", "events": events}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(InputTests)
    if not unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful():
        sys.exit(1)
    if args.model:
        result = json.dumps(smoke(args.model), ensure_ascii=False, indent=2)
        print(result)
        if args.output:
            args.output.write_text(result + "\n")
