use serde::Deserialize;
use serde_json::Value;
use tauri::AppHandle;

use crate::config::AnalyticsTier;

pub const ANALYTICS_TIER: AnalyticsTier = AnalyticsTier::UltraMinimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Detail {
    Launch,
    Core,
    Standard,
    Full,
}

// Compatibility entry points deliberately ignore legacy opt-in preferences.
pub struct SentryGuard;

pub fn init_error_reporting(_app: &AppHandle) -> SentryGuard {
    SentryGuard
}

pub fn flush_error_reporting(_app: &AppHandle) {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendError {
    pub kind: String,
    pub name: String,
    pub message: String,
    #[serde(default)]
    pub stack: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub line: Option<u64>,
    #[serde(default)]
    pub column: Option<u64>,
    #[serde(default)]
    pub window: Option<String>,
}

pub fn capture_frontend_error(_app: &AppHandle, _report: FrontendError) {}

pub fn track(_app: &AppHandle, _event: &str, _props: Option<Value>) {}

pub fn track_at(_app: &AppHandle, _detail: Detail, _event: &str, _props: Option<Value>) {}

pub fn mark_start() {}

pub fn report_exit(_app: &AppHandle, _reason: &'static str) {}

pub fn duration_bucket(ms: u64) -> &'static str {
    match ms {
        0..=9_999 => "<10s",
        10_000..=59_999 => "<1m",
        60_000..=299_999 => "<5m",
        300_000..=899_999 => "<15m",
        900_000..=3_599_999 => "<60m",
        _ => ">=60m",
    }
}

pub fn first_text_bucket(ms: u64) -> &'static str {
    match ms {
        0..=500 => "<=0.5s",
        501..=1_000 => "<=1s",
        1_001..=2_000 => "<=2s",
        2_001..=5_000 => "<=5s",
        5_001..=10_000 => "<=10s",
        _ => ">10s",
    }
}

pub fn sample_rate_bucket(hz: u32) -> &'static str {
    match hz {
        0 => "unknown",
        1..=16_000 => "<=16k",
        16_001..=32_000 => "<=32k",
        32_001..=44_100 => "44.1k",
        44_101..=48_000 => "48k",
        _ => ">48k",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_buckets_cover_boundaries() {
        assert_eq!(duration_bucket(0), "<10s");
        assert_eq!(duration_bucket(9_999), "<10s");
        assert_eq!(duration_bucket(10_000), "<1m");
        assert_eq!(duration_bucket(59_999), "<1m");
        assert_eq!(duration_bucket(60_000), "<5m");
        assert_eq!(duration_bucket(3_599_999), "<60m");
        assert_eq!(duration_bucket(3_600_000), ">=60m");
    }

    #[test]
    fn first_text_buckets_cover_boundaries() {
        assert_eq!(first_text_bucket(0), "<=0.5s");
        assert_eq!(first_text_bucket(500), "<=0.5s");
        assert_eq!(first_text_bucket(501), "<=1s");
        assert_eq!(first_text_bucket(1_000), "<=1s");
        assert_eq!(first_text_bucket(2_001), "<=5s");
        assert_eq!(first_text_bucket(10_001), ">10s");
    }

    #[test]
    fn sample_rate_buckets_cover_common_rates() {
        assert_eq!(sample_rate_bucket(0), "unknown");
        assert_eq!(sample_rate_bucket(16_000), "<=16k");
        assert_eq!(sample_rate_bucket(44_100), "44.1k");
        assert_eq!(sample_rate_bucket(48_000), "48k");
        assert_eq!(sample_rate_bucket(96_000), ">48k");
    }
}
