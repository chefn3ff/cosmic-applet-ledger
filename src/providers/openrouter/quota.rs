// SPDX-License-Identifier: MPL-2.0

use crate::error::OpenRouterError;
use crate::model::{
    DayTokenUsage, ModelTokenUsage, ProviderId, ProviderIdentity, UsageHeadline, UsageSnapshot,
    UsageWindow,
};
use chrono::{DateTime, Local, Utc};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreditsResponse {
    pub data: Option<CreditsData>,
    pub total_credits: Option<f64>,
    pub total_usage: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct CreditsData {
    pub total_credits: Option<f64>,
    pub total_usage: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct ActivityResponse {
    pub data: Option<Vec<ActivityItem>>,
    pub rows: Option<Vec<ActivityItem>>,
}

#[derive(Debug, Deserialize)]
pub struct ActivityItem {
    pub date: Option<String>,
    pub model: Option<String>,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub usage: Option<f64>,
    pub byok_usage_inference: Option<f64>,
    #[allow(dead_code)]
    pub total_tokens: Option<u64>,
    #[allow(dead_code)]
    pub tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct KeyResponse {
    data: Option<KeyData>,
}

#[derive(Debug, Deserialize)]
struct KeyData {
    label: Option<String>,
    limit: Option<f64>,
    limit_remaining: Option<f64>,
    #[allow(dead_code)]
    usage: Option<f64>,
    usage_daily: Option<f64>,
    #[allow(dead_code)]
    usage_monthly: Option<f64>,
    usage_weekly: Option<f64>,
    is_free_tier: Option<bool>,
    #[serde(default)]
    is_management_key: bool,
}

#[derive(Debug, Deserialize)]
struct KeysListResponse {
    data: Option<Vec<KeysListItem>>,
}

#[derive(Debug, Deserialize)]
struct KeysListItem {
    usage_daily: Option<f64>,
    usage_weekly: Option<f64>,
}

/// Account-wide (daily, weekly) spend summed across every key returned by
/// `/api/v1/keys`. Only available to management keys.
fn account_usage_from_keys(body: &str) -> Option<(f64, f64)> {
    let items = serde_json::from_str::<KeysListResponse>(body).ok()?.data?;
    Some(items.iter().fold((0.0, 0.0), |(daily, weekly), item| {
        (
            daily + item.usage_daily.unwrap_or(0.0).max(0.0),
            weekly + item.usage_weekly.unwrap_or(0.0).max(0.0),
        )
    }))
}

/// Management keys can't run inference, so their own `usage_*` counters stay
/// at zero; account spend has to come from the key list instead.
pub fn is_management_key(body: &str) -> bool {
    serde_json::from_str::<KeyResponse>(body)
        .ok()
        .and_then(|response| response.data)
        .is_some_and(|data| data.is_management_key)
}

/// Share of the credit available at the start of a period that has been spent
/// during it: `spent / (spent + balance still left)`.
fn spend_share(spent: f64, balance: f64) -> f32 {
    let available = spent + balance;
    if available <= 0.0 {
        return 0.0;
    }
    ((spent / available) * 100.0).clamp(0.0, 100.0) as f32
}

/// Analytics metric values come back as either JSON strings or numbers.
fn metric_u64(row: &serde_json::Value, name: &str) -> u64 {
    match row.get(name) {
        Some(serde_json::Value::String(value)) => {
            value.parse::<f64>().map_or(0, |v| v.max(0.0) as u64)
        }
        Some(value) => value.as_f64().map_or(0, |v| v.max(0.0) as u64),
        None => 0,
    }
}

fn analytics_rows(body: &str) -> Option<Vec<serde_json::Value>> {
    let value = serde_json::from_str::<serde_json::Value>(body).ok()?;
    value.get("data")?.get("data")?.as_array().cloned()
}

/// "google/gemini-3.8-flash-20260902" -> "google/gemini-3.8-flash", matching
/// the model names `/activity` reports.
fn strip_model_date_suffix(model: &str) -> &str {
    match model.rsplit_once('-') {
        Some((base, suffix)) if suffix.len() == 8 && suffix.chars().all(|c| c.is_ascii_digit()) => {
            base
        }
        _ => model,
    }
}

/// Replaces the activity-derived token breakdowns with `/analytics/query`
/// results, which (unlike `/activity`) include the current day.
///
/// `hourly_body` holds `tokens_total` per UTC hour; hours are bucketed into
/// local days so "Today" follows the user's clock. `model_body` holds
/// `tokens_total` per model over the same window.
pub fn apply_analytics(
    snapshot: &mut UsageSnapshot,
    hourly_body: Option<&str>,
    model_body: Option<&str>,
    today: chrono::NaiveDate,
) {
    use std::collections::HashMap;

    if let Some(rows) = hourly_body.and_then(analytics_rows) {
        let mut day_totals: HashMap<chrono::NaiveDate, u64> = HashMap::new();
        for row in &rows {
            let Some(hour) = row.get("date__hour").and_then(|h| h.as_str()) else {
                continue;
            };
            let Ok(hour) = chrono::NaiveDateTime::parse_from_str(hour, "%Y-%m-%d %H:%M:%S") else {
                continue;
            };
            let local_date = hour.and_utc().with_timezone(&Local).date_naive();
            *day_totals.entry(local_date).or_default() += metric_u64(row, "tokens_total");
        }
        snapshot.tokens_by_day = (0..7)
            .rev()
            .map(|offset| {
                let date = today - chrono::Duration::days(offset);
                let is_today = offset == 0;
                DayTokenUsage {
                    date: date.format("%Y-%m-%d").to_string(),
                    day_label: if is_today {
                        "Today".to_string()
                    } else {
                        date.format("%a").to_string()
                    },
                    token_count: day_totals.get(&date).copied().unwrap_or(0),
                    is_today,
                }
            })
            .collect();
    }

    if let Some(rows) = model_body.and_then(analytics_rows) {
        let mut model_totals: HashMap<String, u64> = HashMap::new();
        for row in &rows {
            let Some(model) = row.get("model").and_then(|m| m.as_str()) else {
                continue;
            };
            *model_totals
                .entry(strip_model_date_suffix(model).to_string())
                .or_default() += metric_u64(row, "tokens_total");
        }
        let mut models: Vec<_> = model_totals
            .into_iter()
            .filter(|(_, tokens)| *tokens > 0)
            .map(|(model_name, token_count)| ModelTokenUsage {
                model_name,
                token_count,
            })
            .collect();
        models.sort_by_key(|model| std::cmp::Reverse(model.token_count));
        models.truncate(5);
        snapshot.tokens_by_model = models;
    }
}

pub fn parse(
    body: &str,
    credits_body: Option<&str>,
    activity_body: Option<&str>,
    keys_body: Option<&str>,
    updated_at: DateTime<Utc>,
) -> Result<UsageSnapshot, OpenRouterError> {
    let response: KeyResponse = serde_json::from_str(body).map_err(OpenRouterError::DecodeUsage)?;
    let data = response.data.ok_or(OpenRouterError::InvalidEnvelope)?;

    let mut usage_daily = data.usage_daily.unwrap_or(0.0).max(0.0);
    let mut usage_weekly = data.usage_weekly.unwrap_or(0.0).max(0.0);
    if let Some((daily, weekly)) = keys_body.and_then(account_usage_from_keys) {
        usage_daily = usage_daily.max(daily);
        usage_weekly = usage_weekly.max(weekly);
    }
    let limit = data.limit;
    let limit_remaining = data.limit_remaining;

    let mut credits_remaining: Option<f64> = None;
    if let Some(c_body) = credits_body
        && let Ok(c_resp) = serde_json::from_str::<CreditsResponse>(c_body)
    {
        let total = c_resp
            .data
            .as_ref()
            .and_then(|d| d.total_credits)
            .or(c_resp.total_credits);
        let used = c_resp
            .data
            .as_ref()
            .and_then(|d| d.total_usage)
            .or(c_resp.total_usage);
        if let (Some(total), Some(used)) = (total, used) {
            credits_remaining = Some((total - used).max(0.0));
        }
    }

    // Parse activity data if available
    let mut tokens_by_day = Vec::new();
    let mut tokens_by_model = Vec::new();
    let mut activity_daily_cost = 0.0;
    let mut activity_weekly_cost = 0.0;

    if let Some(a_body) = activity_body
        && let Ok(a_resp) = serde_json::from_str::<ActivityResponse>(a_body)
        && let Some(items) = a_resp.data.or(a_resp.rows)
    {
        use std::collections::HashMap;
        let mut day_map: HashMap<String, u64> = HashMap::new();
        let mut model_map: HashMap<String, u64> = HashMap::new();

        let today = Local::now().date_naive();
        let today_str = today.format("%Y-%m-%d").to_string();
        let utc_today = updated_at.date_naive();
        let utc_today_str = utc_today.format("%Y-%m-%d").to_string();
        let seven_days_ago = today - chrono::Duration::days(7);

        for item in &items {
            let prompt = item.prompt_tokens.unwrap_or(0);
            let completion = item.completion_tokens.unwrap_or(0);
            let reasoning = item.reasoning_tokens.unwrap_or(0);
            let total = if prompt + completion + reasoning > 0 {
                prompt + completion + reasoning
            } else {
                item.total_tokens.or(item.tokens).unwrap_or(0)
            };
            let date_str = item
                .date
                .as_deref()
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            if !date_str.is_empty() {
                *day_map.entry(date_str).or_insert(0) += total;
            }
            if let Some(ref model) = item.model {
                *model_map.entry(model.clone()).or_insert(0) += total;
            }
        }

        for item in items {
            let cost = item.usage.or(item.byok_usage_inference).unwrap_or(0.0);
            let date_str = item
                .date
                .as_deref()
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            if !date_str.is_empty() {
                if date_str == today_str || date_str == utc_today_str {
                    activity_daily_cost += cost;
                }
                if let Ok(parsed_date) = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                    && parsed_date >= seven_days_ago
                    && (parsed_date <= today || parsed_date <= utc_today)
                {
                    activity_weekly_cost += cost;
                }
            }
        }

        let mut sorted_days: Vec<(String, u64)> = day_map.into_iter().collect();
        sorted_days.sort();

        for (date_str, count) in sorted_days.into_iter().rev().take(7).rev() {
            let is_today = date_str == today_str;
            let day_label = if is_today {
                "Today".to_string()
            } else if let Ok(parsed) = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
                parsed.format("%a").to_string()
            } else {
                date_str.clone()
            };
            tokens_by_day.push(DayTokenUsage {
                date: date_str,
                day_label,
                token_count: count,
                is_today,
            });
        }

        let mut sorted_models: Vec<(String, u64)> = model_map.into_iter().collect();
        sorted_models.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        for (model_name, count) in sorted_models.into_iter().take(5) {
            tokens_by_model.push(ModelTokenUsage {
                model_name,
                token_count: count,
            });
        }
    }

    if usage_daily == 0.0 && activity_daily_cost > 0.0 {
        usage_daily = activity_daily_cost;
    }
    if usage_weekly == 0.0 && activity_weekly_cost > 0.0 {
        usage_weekly = activity_weekly_cost;
    }

    // Build windows:
    // Window 1: Daily usage (today's burn in USD)
    // Window 2: Weekly usage
    let mut windows = Vec::new();

    let daily_percent = match (limit, limit_remaining) {
        (Some(l), Some(r)) if l > 0.0 => {
            let used = (l - r).max(0.0);
            ((used / l) * 100.0) as f32
        }
        _ => match credits_remaining {
            Some(balance) => spend_share(usage_daily, balance),
            None if usage_daily > 0.0 => ((usage_daily / 10.0) * 100.0).clamp(1.0, 100.0) as f32,
            None => 0.0,
        },
    };

    windows.push(UsageWindow {
        label: "Session".to_string(),
        used_percent: daily_percent.clamp(0.0, 100.0),
        reset_at: None,
        window_seconds: Some(86400),
        reset_description: Some(format!("${:.2}", usage_daily)),
        group: None,
    });

    let weekly_percent = match (limit, limit_remaining) {
        (Some(l), Some(r)) if l > 0.0 => {
            let used = (l - r).max(0.0);
            ((used / l) * 100.0) as f32
        }
        _ => match credits_remaining {
            Some(balance) => spend_share(usage_weekly, balance),
            None if usage_weekly > 0.0 => ((usage_weekly / 50.0) * 100.0).clamp(1.0, 100.0) as f32,
            None => 0.0,
        },
    };

    windows.push(UsageWindow {
        label: "Weekly".to_string(),
        used_percent: weekly_percent.clamp(0.0, 100.0),
        reset_at: None,
        window_seconds: Some(604800),
        reset_description: Some(format!("${:.2}", usage_weekly)),
        group: None,
    });

    // Format plan string with remaining balance if available
    let plan = if let Some(remaining) = credits_remaining.or(limit_remaining) {
        Some(format!("${:.2} balance", remaining))
    } else if data.is_free_tier.unwrap_or(false) {
        Some("Free Tier".to_string())
    } else {
        Some("Pay As You Go".to_string())
    };

    Ok(UsageSnapshot {
        provider: ProviderId::OpenRouter,
        source: "API Key".to_string(),
        updated_at,
        headline: UsageHeadline(0),
        windows,
        provider_cost: None,
        extra_usage: None,
        identity: ProviderIdentity {
            email: None,
            account_id: None,
            plan,
            display_name: data.label,
        },
        tokens_by_day,
        tokens_by_model,
        credits_by_key: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANAGEMENT_KEY: &str = r#"{"data":{"label":"mgmt","is_management_key":true,"limit":null,"limit_remaining":null,"usage":0,"usage_daily":0,"usage_weekly":0,"is_free_tier":false}}"#;

    #[test]
    fn management_key_uses_account_spend_from_key_list() {
        let keys = r#"{"data":[{"usage_daily":15.03,"usage_weekly":50.1},{"usage_daily":1.0,"usage_weekly":2.0}]}"#;
        assert!(is_management_key(MANAGEMENT_KEY));
        let snapshot = parse(MANAGEMENT_KEY, None, None, Some(keys), Utc::now()).unwrap();
        assert_eq!(
            snapshot.windows[0].reset_description.as_deref(),
            Some("$16.03")
        );
        assert_eq!(
            snapshot.windows[1].reset_description.as_deref(),
            Some("$52.10")
        );
    }

    #[test]
    fn session_bar_measures_spend_against_available_credit() {
        let credits = r#"{"data":{"total_credits":75,"total_usage":58.85}}"#;
        let keys = r#"{"data":[{"usage_daily":15.0,"usage_weekly":50.0}]}"#;
        let snapshot = parse(MANAGEMENT_KEY, Some(credits), None, Some(keys), Utc::now()).unwrap();
        // balance 16.15: today 15 / 31.15, week 50 / 66.15
        assert!((snapshot.windows[0].used_percent - 48.2).abs() < 0.1);
        assert!((snapshot.windows[1].used_percent - 75.6).abs() < 0.1);
    }

    #[test]
    fn analytics_fills_today_and_models() {
        let mut snapshot = parse(MANAGEMENT_KEY, None, None, None, Utc::now()).unwrap();
        let now = Utc::now();
        let hour = |dt: DateTime<Utc>| dt.format("%Y-%m-%d %H:00:00").to_string();
        let hourly = format!(
            r#"{{"data":{{"data":[{{"date__hour":"{}","tokens_total":"1200"}},{{"date__hour":"{}","tokens_total":34}}]}}}}"#,
            hour(now),
            hour(now - chrono::Duration::days(3)),
        );
        let models = r#"{"data":{"data":[{"model":"google/gemini-3.8-flash-20260902","tokens_total":"900"},{"model":"anthropic/claude-sonnet-5-20260630","tokens_total":"50"}]}}"#;
        let today = now.with_timezone(&Local).date_naive();
        apply_analytics(&mut snapshot, Some(&hourly), Some(models), today);

        assert_eq!(snapshot.tokens_by_day.len(), 7);
        let last = snapshot.tokens_by_day.last().unwrap();
        assert!(last.is_today);
        assert_eq!(last.token_count, 1200);
        assert_eq!(
            snapshot
                .tokens_by_day
                .iter()
                .map(|d| d.token_count)
                .sum::<u64>(),
            1234
        );
        assert_eq!(
            snapshot.tokens_by_model[0].model_name,
            "google/gemini-3.8-flash"
        );
        assert_eq!(snapshot.tokens_by_model[0].token_count, 900);
    }

    #[test]
    fn inference_key_is_not_management_key() {
        let body = r#"{"data":{"label":"k","usage_daily":3.5,"usage_weekly":7.0}}"#;
        assert!(!is_management_key(body));
        let snapshot = parse(body, None, None, None, Utc::now()).unwrap();
        assert_eq!(
            snapshot.windows[0].reset_description.as_deref(),
            Some("$3.50")
        );
    }
}
