use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use super::{apply_zh_script, degenerate_translation, TranslateCaps, TranslateError, Translator};
use crate::config::{base_lang, TranslateConfig, ZhScript};
use crate::i18n::{tf, tr, Key};

pub const MAX_TEXT_CHARS: usize = 2000;
#[cfg(not(windows))]
const WORKER: &str = include_str!("hymt2_worker.py");
#[cfg(windows)]
const WORKER: &str = include_str!("../../../scripts/windows-hymt2-worker.py");
#[cfg(windows)]
use super::hymt2_windows::Process;
const WEIGHTS: &str = "model.safetensors";
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
static WORKER_PID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn terminate_worker() {
    #[cfg(windows)]
    super::hymt2_windows::terminate_worker();
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        let pid = WORKER_PID.swap(0, std::sync::atomic::Ordering::SeqCst);
        if pid != 0 {
            unsafe {
                libc::kill(pid as i32, libc::SIGKILL);
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeConfig {
    pub python: PathBuf,
    #[serde(default)]
    pub allow_cpu: bool,
}

pub fn supported_pair(source: &str, target: &str) -> bool {
    matches!(base_lang(source), "en" | "ru") && base_lang(target) == "zh"
}

pub fn validate_text(text: &str) -> Result<&str, String> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > MAX_TEXT_CHARS || text.contains('\0') {
        return Err(tr(Key::PreviewInvalidText).into());
    }
    Ok(text)
}

fn read_json(path: &Path) -> Result<Value, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn runtime_config(root: &Path) -> Result<RuntimeConfig, String> {
    let runtime: RuntimeConfig = serde_json::from_value(read_json(&root.join("runtime.json"))?)
        .map_err(|e| format!("runtime.json: {e}"))?;
    if !runtime.python.is_absolute() || !runtime.python.is_file() {
        return Err(tr(Key::HymtRuntimeInvalid).into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(&runtime.python)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode()
            & 0o111
            == 0
        {
            return Err(tr(Key::HymtRuntimeInvalid).into());
        }
    }
    Ok(runtime)
}

// Inspect the real tensor container without hashing 1 GB every time the settings page refreshes.
fn validate_weights(path: &Path) -> Result<(), String> {
    let mut file = std::fs::File::open(path).map_err(|e| format!("{WEIGHTS}: {e}"))?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    let mut length = [0u8; 8];
    file.read_exact(&mut length)
        .map_err(|e| format!("{WEIGHTS}: {e}"))?;
    let header_len = u64::from_le_bytes(length);
    if header_len == 0 || header_len > 16 * 1024 * 1024 || header_len + 8 >= size {
        return Err(format!("{WEIGHTS}: invalid header size"));
    }
    let mut header = vec![0u8; header_len as usize];
    file.read_exact(&mut header).map_err(|e| e.to_string())?;
    let header: Value = serde_json::from_slice(&header).map_err(|e| e.to_string())?;
    let tensors = header.as_object().ok_or("invalid tensor header")?;
    let mut count = 0;
    for (name, tensor) in tensors {
        if name == "__metadata__" {
            continue;
        }
        let offsets = tensor["data_offsets"]
            .as_array()
            .ok_or("missing tensor offsets")?;
        if offsets.len() != 2 {
            return Err("invalid tensor offsets".into());
        }
        let start = offsets[0].as_u64().ok_or("invalid tensor start")?;
        let end = offsets[1].as_u64().ok_or("invalid tensor end")?;
        if start >= end || end > size - header_len - 8 {
            return Err("truncated tensor data".into());
        }
        count += 1;
    }
    if count == 0 {
        return Err("empty tensor container".into());
    }
    Ok(())
}

pub fn validate_install(root: &Path) -> Result<(), String> {
    if !cfg!(any(
        windows,
        all(target_os = "macos", target_arch = "aarch64")
    )) {
        return Err(tr(Key::HymtPlatform).into());
    }
    runtime_config(root)?;
    let config = read_json(&root.join("config.json"))?;
    if config["model_type"] != "hunyuan_v1_dense" {
        return Err("config.json: expected hunyuan_v1_dense".into());
    }
    #[cfg(not(windows))]
    if config["quantization"]["bits"] != 4 {
        return Err("config.json: expected 4-bit quantization".into());
    }
    #[cfg(windows)]
    if config.get("quantization").is_some() || config.get("quantization_config").is_some() {
        return Err(
            "Windows requires the official unquantized BF16 model, not MLX/FP8 weights".into(),
        );
    }
    if std::fs::read_to_string(root.join("chat_template.jinja"))
        .map_err(|e| format!("chat_template.jinja: {e}"))?
        .trim()
        .is_empty()
    {
        return Err("chat_template.jinja: empty template".into());
    }
    let tokenizer = read_json(&root.join("tokenizer.json"))?;
    if !tokenizer["model"].is_object() {
        return Err("tokenizer.json: missing model".into());
    }
    let tokenizer_config = read_json(&root.join("tokenizer_config.json"))?;
    if !tokenizer_config.is_object() {
        return Err("tokenizer_config.json: invalid object".into());
    }
    validate_weights(&root.join(WEIGHTS))
}

pub struct Hymt2Translator {
    root: PathBuf,
    source: String,
    script: Option<ZhScript>,
    request_id: u64,
    #[cfg(any(windows, all(target_os = "macos", target_arch = "aarch64")))]
    process: Option<Process>,
}

impl Hymt2Translator {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            source: String::new(),
            script: None,
            request_id: 0,
            #[cfg(any(windows, all(target_os = "macos", target_arch = "aarch64")))]
            process: None,
        }
    }
}

impl Translator for Hymt2Translator {
    fn init(&mut self, cfg: &TranslateConfig) -> Result<(), TranslateError> {
        self.shutdown();
        if !supported_pair(&cfg.source_lang, &cfg.target_lang) {
            return Err(TranslateError::UnsupportedPair {
                from: cfg.source_lang.clone(),
                to: cfg.target_lang.clone(),
            });
        }
        validate_install(&self.root).map_err(TranslateError::ModelLoad)?;
        self.source = base_lang(&cfg.source_lang).into();
        self.script = Some(cfg.zh_script);
        #[cfg(any(windows, all(target_os = "macos", target_arch = "aarch64")))]
        {
            let runtime = runtime_config(&self.root).map_err(TranslateError::ModelLoad)?;
            let mut process = Process::spawn(&runtime.python, &self.root, WORKER)
                .map_err(TranslateError::ModelLoad)?;
            let ready = process
                .read_response(std::time::Duration::from_secs(90))
                .map_err(TranslateError::ModelLoad)?;
            if ready["type"] != "ready" || ready["protocol"] != 1 {
                return Err(TranslateError::ModelLoad(response_error(&ready)));
            }
            tracing::info!(device = %ready["device"], torch = %ready["torch"], mlx = %ready["mlx"], "HY-MT2 已在本机加载并预热");
            if let Some(warning) = ready["warning"].as_str() {
                tracing::warn!("HY-MT2: {warning}");
            }
            self.process = Some(process);
            Ok(())
        }
        #[cfg(not(any(windows, all(target_os = "macos", target_arch = "aarch64"))))]
        Err(TranslateError::PlatformUnavailable(
            tr(Key::HymtPlatform).into(),
        ))
    }

    fn capabilities(&self) -> TranslateCaps {
        TranslateCaps {
            incremental: false,
            offline: true,
            typical_latency_ms: 1500,
        }
    }

    fn translate(&mut self, text: &str) -> Result<String, TranslateError> {
        let text = validate_text(text).map_err(TranslateError::Internal)?;
        #[cfg(any(windows, all(target_os = "macos", target_arch = "aarch64")))]
        {
            self.request_id += 1;
            let id = self.request_id;
            let result = (|| {
                let process = self
                    .process
                    .as_mut()
                    .ok_or_else(|| tr(Key::HymtWorkerRestart).to_string())?;
                process.write_request(&serde_json::json!({"op": "translate", "id": id, "text": text, "source": self.source, "target": "zh"}))?;
                let reply = process.read_response(std::time::Duration::from_secs(60))?;
                if reply["id"].as_u64() != Some(id) || reply["type"] != "translation" {
                    return Err(response_error(&reply));
                }
                let output = reply["text"]
                    .as_str()
                    .ok_or_else(|| tr(Key::HymtProtocol).to_string())?;
                let output = apply_zh_script(output.trim(), self.script);
                if let Some(reason) = degenerate_translation(&output) {
                    return Err(reason);
                }
                Ok(output)
            })();
            if result.is_err() {
                self.shutdown();
            }
            result.map_err(TranslateError::Internal)
        }
        #[cfg(not(any(windows, all(target_os = "macos", target_arch = "aarch64"))))]
        {
            let _ = text;
            Err(TranslateError::NotInitialized)
        }
    }

    fn is_ready(&self) -> bool {
        #[cfg(any(windows, all(target_os = "macos", target_arch = "aarch64")))]
        {
            self.process.is_some()
        }
        #[cfg(not(any(windows, all(target_os = "macos", target_arch = "aarch64"))))]
        {
            false
        }
    }

    fn shutdown(&mut self) {
        #[cfg(any(windows, all(target_os = "macos", target_arch = "aarch64")))]
        {
            self.process.take();
        }
    }
}

impl Drop for Hymt2Translator {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn response_error(reply: &Value) -> String {
    tf(
        Key::HymtWorkerError,
        &[reply["error"].as_str().unwrap_or(tr(Key::HymtProtocol))],
    )
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
struct Process {
    child: std::process::Child,
    input: std::process::ChildStdin,
    output: std::process::ChildStdout,
    buffered: Vec<u8>,
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
impl Process {
    fn spawn(python: &Path, root: &Path, worker: &str) -> Result<Self, String> {
        use std::os::fd::AsRawFd;
        use std::process::{Command, Stdio};
        let mut child = Command::new(python)
            .args(["-I", "-u", "-c", worker])
            .arg(root)
            .env("HF_HUB_OFFLINE", "1")
            .env("TRANSFORMERS_OFFLINE", "1")
            .env("HF_HUB_DISABLE_TELEMETRY", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| tf(Key::HymtWorkerError, &[&e.to_string()]))?;
        let input = child.stdin.take().expect("piped stdin");
        let output = child.stdout.take().expect("piped stdout");
        let process = Self {
            child,
            input,
            output,
            buffered: Vec::new(),
        };
        for fd in [process.input.as_raw_fd(), process.output.as_raw_fd()] {
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
            if flags == -1
                || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
        }
        WORKER_PID.store(process.child.id(), std::sync::atomic::Ordering::SeqCst);
        Ok(process)
    }

    fn wait_fd(
        fd: std::os::fd::RawFd,
        events: i16,
        deadline: std::time::Instant,
    ) -> Result<(), String> {
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(tr(Key::HymtTimeout).into());
            }
            let mut fds = libc::pollfd {
                fd,
                events,
                revents: 0,
            };
            let result = unsafe { libc::poll(&mut fds, 1, remaining.as_millis().min(1000) as i32) };
            if result > 0 {
                return Ok(());
            }
            if result < 0
                && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
        }
    }

    fn write_request(&mut self, request: &Value) -> Result<(), String> {
        use std::io::Write;
        use std::os::fd::AsRawFd;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut data = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        data.push(b'\n');
        let mut offset = 0;
        while offset < data.len() {
            Self::wait_fd(self.input.as_raw_fd(), libc::POLLOUT, deadline)?;
            match self.input.write(&data[offset..]) {
                Ok(0) => return Err(tr(Key::HymtWorkerExited).into()),
                Ok(n) => offset += n,
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    continue
                }
                Err(e) => return Err(tf(Key::HymtWorkerError, &[&e.to_string()])),
            }
        }
        Ok(())
    }

    fn read_response(&mut self, timeout: std::time::Duration) -> Result<Value, String> {
        use std::os::fd::AsRawFd;
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if let Some(end) = self.buffered.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = self.buffered.drain(..=end).collect();
                return serde_json::from_slice(&line).map_err(|_| tr(Key::HymtProtocol).into());
            }
            if self.buffered.len() > 128 * 1024 {
                return Err(tr(Key::HymtProtocol).into());
            }
            Self::wait_fd(self.output.as_raw_fd(), libc::POLLIN, deadline)?;
            let mut buf = [0u8; 4096];
            match self.output.read(&mut buf) {
                Ok(0) => return Err(tr(Key::HymtWorkerExited).into()),
                Ok(n) => self.buffered.extend_from_slice(&buf[..n]),
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    continue
                }
                Err(e) => return Err(tf(Key::HymtWorkerError, &[&e.to_string()])),
            }
        }
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
impl Drop for Process {
    fn drop(&mut self) {
        // Killing a private worker also handles a stuck generation; always reap the child.
        let _ = WORKER_PID.compare_exchange(
            self.child.id(),
            0,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        );
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restricts_to_implemented_directions() {
        assert!(supported_pair("ru-RU", "zh-Hant"));
        assert!(supported_pair("en", "zh"));
        assert!(!supported_pair("zh", "en"));
        assert!(!supported_pair("en", "ru"));
        assert!(!supported_pair("fr", "zh"));
    }
    #[test]
    fn text_validation_counts_unicode_characters() {
        assert!(validate_text(&"界".repeat(MAX_TEXT_CHARS)).is_ok());
        assert!(validate_text(&"界".repeat(MAX_TEXT_CHARS + 1)).is_err());
        assert_eq!(
            validate_text("  Привет 🌏\n世界  ").unwrap(),
            "Привет 🌏\n世界"
        );
        assert!(validate_text("\n  ").is_err());
        assert!(validate_text("a\0b").is_err());
    }
    #[test]
    fn missing_runtime_is_not_installed() {
        assert!(runtime_config(Path::new("/nonexistent/hymt2")).is_err());
    }
    #[test]
    fn worker_error_is_preserved() {
        assert!(
            response_error(&serde_json::json!({"error":"test failure"})).contains("test failure")
        );
    }
    #[test]
    fn worker_has_no_network_server_or_remote_code() {
        assert!(WORKER.contains("trust_remote_code") && WORKER.contains("False"));
        assert!(WORKER.contains("local_files_only") && WORKER.contains("True"));
        assert!(!WORKER.contains("http.server"));
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn stub(script: &str) -> Process {
        Process::spawn(Path::new("/usr/bin/python3"), Path::new("/tmp"), script).unwrap()
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn pipe_handles_unicode_multiple_requests_and_reaps_child() {
        let mut process = stub("import sys,json\nprint('{\"type\":\"ready\"}',flush=True)\nfor line in sys.stdin:\n r=json.loads(line); print(json.dumps(r,ensure_ascii=False),flush=True)");
        assert_eq!(
            process
                .read_response(std::time::Duration::from_secs(5))
                .unwrap()["type"],
            "ready"
        );
        for id in 1..=2 {
            let request = serde_json::json!({"id":id,"text":"Привет 🌏\n世界"});
            process.write_request(&request).unwrap();
            assert_eq!(
                process
                    .read_response(std::time::Duration::from_secs(5))
                    .unwrap(),
                request
            );
        }
        let pid = process.child.id() as i32;
        drop(process);
        assert_eq!(
            unsafe { libc::kill(pid, 0) },
            -1,
            "丢弃后子进程必须退出并回收"
        );
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn pipe_reports_timeout_malformed_response_and_early_exit() {
        let mut timeout = stub("import time; time.sleep(60)");
        assert!(timeout
            .read_response(std::time::Duration::from_millis(30))
            .is_err());
        drop(timeout);
        let mut malformed = stub("print('not json',flush=True)");
        assert!(malformed
            .read_response(std::time::Duration::from_secs(5))
            .is_err());
        let mut exited = stub("raise SystemExit(2)");
        assert!(exited
            .read_response(std::time::Duration::from_secs(5))
            .is_err());
    }
    #[test]
    fn validates_real_safetensors_extent() {
        let dir = std::env::temp_dir().join(format!("hymt-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(WEIGHTS);
        let header = br#"{"w":{"data_offsets":[0,4],"dtype":"F32","shape":[1]}}"#;
        let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
        bytes.extend_from_slice(header);
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        std::fs::write(&path, &bytes).unwrap();
        assert!(validate_weights(&path).is_ok());
        bytes.pop();
        std::fs::write(&path, &bytes).unwrap();
        assert!(validate_weights(&path).is_err(), "截断权重不能被标为可用");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
