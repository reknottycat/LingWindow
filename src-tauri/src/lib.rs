pub mod asr;
pub mod audio;
pub mod commands;
pub mod config;
pub mod env_info;
pub mod error;
pub mod i18n;
pub mod lifecycle;
pub mod log;
pub mod models;
pub mod monitor;
pub mod overlay;
pub mod paths;
pub mod permission;
pub mod prefs;
pub mod session;
pub mod shortcut;
pub mod telemetry;
pub mod transcript;
pub mod translate;
pub mod tray;
pub mod update;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager};

pub const EVENT_LOCALE: &str = "app://locale";

static MAIN_TICK_MS: AtomicU64 = AtomicU64::new(0);

fn update_window_titles(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(lifecycle::MAIN_LABEL) {
        let _ = window.set_title(i18n::tr(i18n::Key::AppWindowTitle));
    }
}

pub fn apply_language(app: &AppHandle, language: config::Language) {
    let locale = i18n::Locale::from_language(language);
    if !i18n::set_locale(locale) {
        return;
    }
    update_window_titles(app);
    tray::relocalize(app);
    if let Err(e) = app.emit(EVENT_LOCALE, locale.as_str()) {
        tracing::warn!("广播界面语言失败：{e}");
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn spawn_main_thread_watchdog(app: &AppHandle) {
    MAIN_TICK_MS.store(now_ms(), Ordering::Relaxed);
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("main-thread-watchdog".into())
        .spawn(move || {
            let mut last_warn = 0u64;
            loop {
                if app
                    .run_on_main_thread(|| {
                        MAIN_TICK_MS.store(now_ms(), Ordering::Relaxed);
                    })
                    .is_err()
                {
                    break;
                }

                std::thread::sleep(Duration::from_secs(2));
                let now = now_ms();
                let gap = now.saturating_sub(MAIN_TICK_MS.load(Ordering::Relaxed));
                if gap >= 5000 && now.saturating_sub(last_warn) >= 10_000 {
                    last_warn = now;
                    tracing::warn!(gap_ms = gap, "主线程超过 {}ms 未处理任务", gap);
                }
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("主线程看门狗启动失败：{e}");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    telemetry::mark_start();
    let mut builder = tauri::Builder::default();

    builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
        tracing::info!("检测到第二次启动，唤回已有实例");
        lifecycle::show_main(app);
    }));

    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }

    builder
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(lifecycle::LifecycleState::default())
        .manage(overlay::OverlayState::default())
        .manage(shortcut::HotkeyState::default())
        .manage(models::state::ModelState::default())
        .manage(monitor::MonitorState::default())
        .manage(session::SessionState::default())
        .invoke_handler(tauri::generate_handler![commands::dispatch])
        .on_window_event(lifecycle::on_window_event)
        .on_page_load(|webview, payload| {
            if webview.label() != overlay::LABEL {
                return;
            }
            if !matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
                return;
            }
            overlay::on_page_loaded(webview.app_handle());
        })
        .setup(|app| {
            let handle = app.handle().clone();

            handle.manage(log::LogDir(log::init(&handle)));
            handle.manage(telemetry::init_error_reporting(&handle));

            if let Err(e) = session::init_emitter(&handle) {
                tracing::error!("{e}");
            }
            if let Err(e) = session::init_teardown_executor() {
                tracing::error!("{e}");
            }
            if let Err(e) = session::init_translate_worker() {
                tracing::error!("{e}");
            }
            if let Err(e) = crate::asr::offline::init_worker() {
                tracing::error!("{e}");
            }
            if let Err(e) = commands::init_executors() {
                tracing::error!("{e}");
            }

            spawn_main_thread_watchdog(&handle);

            let cleanup_handle = handle.clone();
            if let Err(e) = std::thread::Builder::new()
                .name("models-cleanup".into())
                .spawn(move || {
                    paths::log_startup_selfcheck(&cleanup_handle);
                    models::install::cleanup_stale(&cleanup_handle);
                })
            {
                tracing::warn!("无法启动模型临时目录清理线程：{e}");
            }

            let saved_prefs = prefs::load(&handle);
            i18n::set_locale(i18n::Locale::from_language(saved_prefs.language));
            update_window_titles(&handle);
            env_info::log_startup(&saved_prefs);
            telemetry::track_at(
                &handle,
                telemetry::Detail::Launch,
                "app_started",
                Some(serde_json::json!({
                    "asr_model_tier": saved_prefs.asr_model_tier.as_str(),
                    "asr_engine": saved_prefs.asr_engine.as_str(),
                    "mt_model_tier": saved_prefs.mt_model_tier.as_str(),
                    "audio_source": saved_prefs.audio_source.as_str(),
                    "translate_engine": saved_prefs.translate_engine.as_str(),
                    "source_lang": saved_prefs.source_lang,
                    "target_lang": saved_prefs.target_lang,
                    "onboarding_done": if saved_prefs.onboarding_done { 1 } else { 0 },
                })),
            );
            if let Err(e) =
                overlay::init(&handle, saved_prefs.overlay, saved_prefs.overlay_geometry)
            {
                tracing::error!("悬浮窗初始化失败：{e}");
            }

            if let Err(e) = tray::init(&handle) {
                tracing::error!("托盘初始化失败：{e}");
            }

            if let Err(e) = shortcut::init(&handle) {
                tracing::error!("全局热键初始化失败：{e}");
            }

            lifecycle::show_main(&handle);

            // Packaged LingWindow never starts audio capture from ambient environment variables.

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match &event {
            tauri::RunEvent::ExitRequested { api, .. } => {
                if lifecycle::on_run_event(app, &event) {
                    api.prevent_exit();
                }
            }
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => lifecycle::show_main(app),
            tauri::RunEvent::Exit => {
                translate::hymt2::terminate_worker();
                telemetry::report_exit(app, "os");
                telemetry::flush_error_reporting(app);
            }
            _ => {}
        });
}
