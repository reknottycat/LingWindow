use std::fs::File;
use std::io::{Read, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use windows_sys::Win32::Foundation::{SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::System::Pipes::{CreatePipe, PeekNamedPipe};
use windows_sys::Win32::System::Threading::{TerminateProcess, CREATE_NO_WINDOW};

use crate::i18n::{tf, tr, Key};

const MAX_REQUEST: usize = 32 * 1024;
const MAX_RESPONSE: usize = 128 * 1024;
static WORKERS: Mutex<Vec<Arc<OwnedHandle>>> = Mutex::new(Vec::new());

pub(super) fn terminate_worker() {
    // Retained handles identify the original processes even after their PIDs are reused.
    let workers = WORKERS.lock().unwrap_or_else(|e| e.into_inner());
    for handle in workers.iter() {
        unsafe { TerminateProcess(handle.as_raw_handle() as HANDLE, 1) };
    }
}

pub(super) struct Process {
    child: Child,
    input: File,
    output: ChildStdout,
    handle: Option<Arc<OwnedHandle>>,
    buffered: Vec<u8>,
    awaiting_response: bool,
    failed: bool,
}

fn worker_error(e: std::io::Error) -> String {
    tf(Key::HymtWorkerError, &[&e.to_string()])
}

fn input_pipe() -> Result<(OwnedHandle, File), String> {
    let mut read = std::ptr::null_mut();
    let mut write = std::ptr::null_mut();
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 1,
    };
    if unsafe { CreatePipe(&mut read, &mut write, &attributes, 64 * 1024) } == 0 {
        return Err(worker_error(std::io::Error::last_os_error()));
    }
    // Own both ends immediately, including on the SetHandleInformation failure path.
    let read = unsafe { OwnedHandle::from_raw_handle(read) };
    let write = unsafe { OwnedHandle::from_raw_handle(write) };
    if unsafe { SetHandleInformation(write.as_raw_handle() as HANDLE, HANDLE_FLAG_INHERIT, 0) } == 0
    {
        return Err(worker_error(std::io::Error::last_os_error()));
    }
    Ok((read, File::from(write)))
}

impl Process {
    pub(super) fn spawn(python: &Path, root: &Path, worker: &str) -> Result<Self, String> {
        let (read, input) = input_pipe()?;
        let mut child = Command::new(python)
            // Isolated mode ignores PYTHONIOENCODING, so force UTF-8 with -X too.
            .args(["-I", "-X", "utf8", "-u", "-c", worker])
            .arg(root)
            .env("PYTHONIOENCODING", "utf-8")
            .env("HF_HUB_OFFLINE", "1")
            .env("TRANSFORMERS_OFFLINE", "1")
            .env("HF_DATASETS_OFFLINE", "1")
            .env("HF_HUB_DISABLE_TELEMETRY", "1")
            .env("TOKENIZERS_PARALLELISM", "false")
            .stdin(Stdio::from(read))
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(worker_error)?;
        let output = child.stdout.take().expect("piped stdout");
        let mut process = Self {
            child,
            input,
            output,
            handle: None,
            buffered: Vec::new(),
            awaiting_response: true,
            failed: false,
        };
        let handle = Arc::new(
            std::os::windows::io::AsHandle::as_handle(&process.child)
                .try_clone_to_owned()
                .map_err(worker_error)?,
        );
        WORKERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(handle.clone());
        process.handle = Some(handle);
        Ok(process)
    }

    pub(super) fn write_request(&mut self, request: &Value) -> Result<(), String> {
        if self.failed || self.awaiting_response || !self.buffered.is_empty() {
            return Err(tr(Key::HymtProtocol).into());
        }
        let mut data = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        data.push(b'\n');
        if data.len() >= MAX_REQUEST {
            return Err(tr(Key::HymtProtocol).into());
        }
        if self.child.try_wait().map_err(worker_error)?.is_some() {
            self.failed = true;
            return Err(tr(Key::HymtWorkerExited).into());
        }
        // One request smaller than half the explicit pipe capacity. The previous
        // response acknowledges consumption, so no writer thread or blocking flush is needed.
        self.awaiting_response = true;
        if let Err(e) = self.input.write_all(&data) {
            self.failed = true;
            return Err(worker_error(e));
        }
        Ok(())
    }

    pub(super) fn read_response(&mut self, timeout: Duration) -> Result<Value, String> {
        if self.failed || !self.awaiting_response {
            return Err(tr(Key::HymtProtocol).into());
        }
        let result = self.read_line(timeout);
        match result {
            Ok(value) => {
                self.awaiting_response = false;
                Ok(value)
            }
            Err(error) => {
                self.failed = true;
                Err(error)
            }
        }
    }

    fn read_line(&mut self, timeout: Duration) -> Result<Value, String> {
        let started = Instant::now();
        loop {
            if let Some(end) = self.buffered.iter().position(|b| *b == b'\n') {
                if end + 1 > MAX_RESPONSE {
                    return Err(tr(Key::HymtProtocol).into());
                }
                let line: Vec<u8> = self.buffered.drain(..=end).collect();
                let value: Value =
                    serde_json::from_slice(&line).map_err(|_| tr(Key::HymtProtocol).to_string())?;
                if !value.is_object() {
                    return Err(tr(Key::HymtProtocol).into());
                }
                return Ok(value);
            }
            if self.buffered.len() >= MAX_RESPONSE {
                return Err(tr(Key::HymtProtocol).into());
            }
            if started.elapsed() >= timeout {
                return Err(tr(Key::HymtTimeout).into());
            }
            let mut available = 0;
            let peeked = unsafe {
                PeekNamedPipe(
                    self.output.as_raw_handle() as HANDLE,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                )
            };
            if peeked == 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error()
                    == Some(windows_sys::Win32::Foundation::ERROR_BROKEN_PIPE as i32)
                {
                    return Err(tr(Key::HymtWorkerExited).into());
                }
                return Err(worker_error(error));
            }
            if available > 0 {
                let mut buf = [0u8; 4096];
                let count = (available as usize)
                    .min(buf.len())
                    .min(MAX_RESPONSE - self.buffered.len());
                // This is the only reader, and it never reads beyond bytes reported available.
                let read = self.output.read(&mut buf[..count]).map_err(worker_error)?;
                if read == 0 {
                    return Err(tr(Key::HymtWorkerExited).into());
                }
                self.buffered.extend_from_slice(&buf[..read]);
            } else {
                if self.child.try_wait().map_err(worker_error)?.is_some() {
                    return Err(tr(Key::HymtWorkerExited).into());
                }
                std::thread::sleep(
                    Duration::from_millis(5).min(timeout.saturating_sub(started.elapsed())),
                );
            }
        }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        if let Some(handle) = &self.handle {
            WORKERS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retain(|registered| !Arc::ptr_eq(registered, handle));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
    use windows_sys::Win32::System::Threading::WaitForSingleObject;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn stub(script: &str) -> Process {
        let python = std::env::var_os("PYTHON").unwrap_or_else(|| "python".into());
        Process::spawn(Path::new(&python), &std::env::temp_dir(), script)
            .expect("Windows 管道测试需要可用的 Python")
    }

    #[test]
    fn unicode_repeated_requests_and_cleanup() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut process = stub("import sys,json\nprint('{\"type\":\"ready\"}',flush=True)\nfor line in sys.stdin:\n r=json.loads(line); print(json.dumps(r,ensure_ascii=False),flush=True)");
        let handle = process.handle.as_ref().unwrap().clone();
        assert_eq!(
            process.read_response(Duration::from_secs(10)).unwrap()["type"],
            "ready"
        );
        for id in 1..=20 {
            let request = serde_json::json!({"id":id,"text":"Привет 🌏\n世界"});
            process.write_request(&request).unwrap();
            assert_eq!(
                process.read_response(Duration::from_secs(10)).unwrap(),
                request
            );
        }
        drop(process);
        assert_eq!(
            unsafe { WaitForSingleObject(handle.as_raw_handle() as HANDLE, 0) },
            WAIT_OBJECT_0,
            "丢弃后必须回收子进程"
        );
    }

    #[test]
    fn timeout_malformed_early_exit_and_response_limit() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        for script in [
            "import time; time.sleep(60)",
            "print('not json',flush=True)",
            "raise SystemExit(2)",
            "print('[' + '0,'*70000 + '0]',flush=True)",
        ] {
            let mut process = stub(script);
            let timeout = if script.contains("sleep") {
                Duration::from_millis(30)
            } else {
                Duration::from_secs(10)
            };
            assert!(
                process.read_response(timeout).is_err(),
                "异常响应必须被拒绝"
            );
            assert!(
                process.write_request(&serde_json::json!({})).is_err(),
                "异常后不可继续复用管道"
            );
        }
    }

    #[test]
    fn request_limit_and_single_outstanding_request() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut process =
            stub("import time\nprint('{\"type\":\"ready\"}',flush=True)\ntime.sleep(60)");
        assert!(
            process.write_request(&serde_json::json!({})).is_err(),
            "就绪前不可写入"
        );
        process.read_response(Duration::from_secs(10)).unwrap();
        assert!(
            process
                .write_request(&serde_json::json!({"text":"界".repeat(MAX_REQUEST)}))
                .is_err(),
            "拒绝超大请求"
        );
        let started = Instant::now();
        process
            .write_request(&serde_json::json!({"text":"x".repeat(MAX_REQUEST - 32)}))
            .unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "子进程不读取时单次受限写入仍应立即返回"
        );
        assert!(
            process.write_request(&serde_json::json!({})).is_err(),
            "不可堆积请求"
        );
    }
    #[test]
    fn lifecycle_termination_uses_retained_process_handles() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut process =
            stub("import time\nprint('{\"type\":\"ready\"}',flush=True)\ntime.sleep(60)");
        process.read_response(Duration::from_secs(10)).unwrap();
        let handle = process.handle.as_ref().unwrap().clone();
        terminate_worker();
        assert_eq!(
            unsafe { WaitForSingleObject(handle.as_raw_handle() as HANDLE, 5000) },
            WAIT_OBJECT_0,
            "停止操作必须终止已登记的私有进程"
        );
        drop(process);
        assert!(
            WORKERS.lock().unwrap_or_else(|e| e.into_inner()).is_empty(),
            "丢弃后必须移除进程句柄登记"
        );
        terminate_worker();
    }
    #[test]
    fn venv_redirector_unicode_and_real_interpreter_cleanup() {
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE};

        struct Venv(std::path::PathBuf);
        impl Drop for Venv {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let python = std::env::var_os("PYTHON").unwrap_or_else(|| "python".into());
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let venv = Venv(
            std::env::temp_dir().join(format!("hymt2 venv 中文 {} {nonce}", std::process::id())),
        );
        let created = Command::new(python)
            .args(["-I", "-m", "venv", "--without-pip", "--copies"])
            .arg(&venv.0)
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .expect("必须能够创建 Windows 原生 Python 虚拟环境");
        assert!(created.success(), "虚拟环境创建必须成功");
        let script = "import sys,json,os\nprint(json.dumps({'type':'ready','pid':os.getpid(),'prefix':sys.prefix}),flush=True)\nfor line in sys.stdin:\n r=json.loads(line); print(json.dumps(r,ensure_ascii=False),flush=True)";
        for explicit_stop in [false, true] {
            let mut process =
                Process::spawn(&venv.0.join("Scripts/python.exe"), &venv.0, script).unwrap();
            let ready = process.read_response(Duration::from_secs(20)).unwrap();
            assert_eq!(
                std::fs::canonicalize(ready["prefix"].as_str().unwrap()).unwrap(),
                std::fs::canonicalize(&venv.0).unwrap(),
                "测试必须运行在刚创建的虚拟环境中"
            );
            let worker_pid = u32::try_from(ready["pid"].as_u64().unwrap()).unwrap();
            // Retain the actual interpreter handle before stopping the redirector;
            // checking only the redirector's PID cannot detect a leaked interpreter.
            let raw = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, worker_pid) };
            assert!(!raw.is_null(), "必须能够保留真实解释器进程句柄");
            let interpreter = unsafe { OwnedHandle::from_raw_handle(raw) };
            let launcher = process.handle.as_ref().unwrap().clone();
            eprintln!(
                "虚拟环境启动器 PID={}，实际解释器 PID={worker_pid}，显式停止={explicit_stop}",
                process.child.id()
            );
            for id in 1..=10 {
                let request = serde_json::json!({"id":id,"text":"Привет 🌏\n世界"});
                process.write_request(&request).unwrap();
                assert_eq!(
                    process.read_response(Duration::from_secs(10)).unwrap(),
                    request
                );
            }
            if explicit_stop {
                terminate_worker();
            }
            drop(process);
            assert_eq!(
                unsafe { WaitForSingleObject(launcher.as_raw_handle() as HANDLE, 5000) },
                WAIT_OBJECT_0,
                "虚拟环境启动器必须退出"
            );
            assert_eq!(
                unsafe { WaitForSingleObject(interpreter.as_raw_handle() as HANDLE, 5000) },
                WAIT_OBJECT_0,
                "真实解释器不可遗留在后台占用 GPU"
            );
        }
    }
}
