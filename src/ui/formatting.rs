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
    if ratio >= 0.80 {
        StatusKind::Error
    } else if ratio >= 0.50 {
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

pub(super) fn format_elapsed_short(duration: Duration) -> String {
    let secs = duration.as_secs_f64();
    if secs < 60.0 {
        format!("{secs:.1}s")
    } else {
        let total = duration.as_secs();
        format!("{}m {}s", total / 60, total % 60)
    }
}

pub(super) fn estimate_tokens_from_chars(chars: usize) -> f64 {
    chars as f64 / 4.0
}

pub(super) fn tokens_per_second(token_chars: usize, since: Duration) -> Option<f64> {
    let secs = since.as_secs_f64();
    if secs < 0.3 {
        return None;
    }
    let tokens = estimate_tokens_from_chars(token_chars).max(1.0);
    Some(tokens / secs)
}
