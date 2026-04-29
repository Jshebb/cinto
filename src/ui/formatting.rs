use std::time::Duration;

use crate::theme::StatusKind;

pub(super) fn compact(value: &str, max_len: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_len || max_len < 4 {
        return value.to_string();
    }

    let prefix = value.chars().take(max_len - 3).collect::<String>();
    format!("{prefix}...")
}

pub(super) fn token_ratio(estimated: usize, max_tokens: u32) -> f64 {
    if max_tokens == 0 {
        return 0.0;
    }

    (estimated as f64 / max_tokens as f64).clamp(0.0, 1.0)
}

pub(super) fn token_status_kind(ratio: f64) -> StatusKind {
    if ratio >= 0.9 {
        StatusKind::Error
    } else if ratio >= 0.72 {
        StatusKind::Warn
    } else {
        StatusKind::Ok
    }
}

pub(super) fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 60 {
        format!("{seconds}s")
    } else {
        format!("{}m {}s", seconds / 60, seconds % 60)
    }
}
