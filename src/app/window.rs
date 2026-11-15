use super::{Duration, Message, Task, UPDATE_RETRY_INITIAL_SECS, UPDATE_RETRY_MAX_SECS, runtime};

pub(super) fn open_url(url: &str) {
    crate::browser::open(url, "link");
}

pub(super) fn update_check_task(attempt: u32) -> Task<Message> {
    Task::perform(
        async { crate::updates::check(&runtime::http_client()).await },
        move |status| cosmic::Action::App(Message::UpdateChecked { status, attempt }),
    )
}

pub(super) fn update_retry_task(attempt: u32, delay: Duration) -> Task<Message> {
    Task::perform(
        async move {
            tokio::time::sleep(delay).await;
            attempt
        },
        |attempt| cosmic::Action::App(Message::RetryUpdateCheck(attempt)),
    )
}

pub(super) fn update_retry_delay(attempt: u32) -> Duration {
    let exponent = attempt.saturating_sub(1).min(10);
    let secs = UPDATE_RETRY_INITIAL_SECS
        .saturating_mul(2_u64.saturating_pow(exponent))
        .min(UPDATE_RETRY_MAX_SECS);
    Duration::from_secs(secs)
}

pub(super) fn format_retry_delay(delay: Duration) -> String {
    let secs = delay.as_secs();
    if secs < 60 {
        return format!("{secs}s");
    }
    let minutes = secs / 60;
    let seconds = secs % 60;
    if seconds == 0 {
        return format!("{minutes}m");
    }
    format!("{minutes}m {seconds}s")
}
