// SPDX-License-Identifier: MPL-2.0

use crate::error::ElevenLabsError;
use crate::model::{
    KeyCreditUsage, ProviderCost, ProviderId, ProviderIdentity, UsageHeadline, UsageSnapshot,
    UsageWindow,
};
use chrono::{DateTime, Months, Utc};
use serde::Deserialize;
use std::collections::HashMap;

/// `GET /v1/user/subscription`. Unknown fields are ignored.
#[derive(Debug, Deserialize)]
struct SubscriptionResponse {
    tier: String,
    character_count: u64,
    character_limit: u64,
    next_character_count_reset_unix: Option<i64>,
    #[allow(dead_code)]
    status: Option<String>,
    character_refresh_period: Option<String>,
}

/// `GET /v1/usage/character-stats`: one series of per-bucket values per key.
#[derive(Debug, Deserialize)]
struct CharacterStatsResponse {
    usage: HashMap<String, Vec<f64>>,
}

/// Key usage and the subscription counter drift by a few tens of credits, so
/// a "no key" remainder only shows once it clears this share of the limit.
const NO_KEY_MIN_SHARE: f64 = 0.01;

/// Start of the current credit period: the next reset minus one refresh
/// period. `None` when the subscription carries no reset time.
pub fn period_start(body: &str) -> Option<DateTime<Utc>> {
    let response: SubscriptionResponse = serde_json::from_str(body).ok()?;
    let reset = DateTime::from_timestamp(response.next_character_count_reset_unix?, 0)?;
    let months = match response.character_refresh_period.as_deref() {
        Some("3_month_period") => 3,
        Some("6_month_period") => 6,
        Some("annual_period") => 12,
        _ => 1,
    };
    reset.checked_sub_months(Months::new(months))
}

/// Fills `credits_by_key` from an `all_api_keys` breakdown, largest first.
/// Credits used outside any key become a trailing `key_name: None` row.
pub fn apply_key_usage(snapshot: &mut UsageSnapshot, body: &str) {
    let Ok(stats) = serde_json::from_str::<CharacterStatsResponse>(body) else {
        return;
    };
    let mut keys: Vec<KeyCreditUsage> = stats
        .usage
        .into_iter()
        .map(|(name, series)| KeyCreditUsage {
            key_name: Some(name),
            credits: series.iter().sum::<f64>().max(0.0).round() as u64,
        })
        .filter(|key| key.credits > 0)
        .collect();
    keys.sort_by(|a, b| b.credits.cmp(&a.credits).then(a.key_name.cmp(&b.key_name)));

    if let Some(ProviderCost {
        used,
        limit: Some(limit),
        ..
    }) = &snapshot.provider_cost
    {
        let by_keys: u64 = keys.iter().map(|key| key.credits).sum();
        let remainder = (*used as u64).saturating_sub(by_keys);
        if remainder as f64 > limit * NO_KEY_MIN_SHARE {
            keys.push(KeyCreditUsage {
                key_name: None,
                credits: remainder,
            });
        }
    }
    snapshot.credits_by_key = keys;
}

/// "creator" -> "Creator". Only the first letter changes, so tiers such as
/// "free" or "growing_business" stay readable.
fn title_case(tier: &str) -> String {
    let mut chars = tier.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

pub fn parse(body: &str, updated_at: DateTime<Utc>) -> Result<UsageSnapshot, ElevenLabsError> {
    let response: SubscriptionResponse =
        serde_json::from_str(body).map_err(ElevenLabsError::DecodeUsage)?;

    let used = response.character_count;
    let limit = response.character_limit;
    let used_percent = if limit == 0 {
        0.0
    } else {
        ((used as f64 / limit as f64) * 100.0).clamp(0.0, 100.0) as f32
    };
    let reset_at = response
        .next_character_count_reset_unix
        .and_then(|secs| DateTime::from_timestamp(secs, 0));

    let plan = Some(title_case(&response.tier)).filter(|plan| !plan.is_empty());

    Ok(UsageSnapshot {
        provider: ProviderId::ElevenLabs,
        source: "API Key".to_string(),
        updated_at,
        headline: UsageHeadline(0),
        windows: vec![UsageWindow {
            label: "Credits".to_string(),
            used_percent,
            reset_at,
            window_seconds: None,
            reset_description: None,
            group: None,
        }],
        provider_cost: Some(ProviderCost {
            used: used as f64,
            limit: Some(limit as f64),
            units: "credits".to_string(),
        }),
        extra_usage: None,
        identity: ProviderIdentity {
            email: None,
            account_id: None,
            plan,
            display_name: None,
        },
        tokens_by_day: Vec::new(),
        tokens_by_model: Vec::new(),
        credits_by_key: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::from_timestamp(1_790_000_000, 0).expect("valid timestamp")
    }

    #[test]
    fn parses_normal_response() {
        let snapshot = parse(
            include_str!("../../../fixtures/elevenlabs/subscription.json"),
            now(),
        )
        .expect("fixture parses");

        assert_eq!(snapshot.provider, ProviderId::ElevenLabs);
        assert_eq!(snapshot.windows.len(), 1);
        let window = &snapshot.windows[0];
        assert_eq!(window.label, "Credits");
        assert!((window.used_percent - 27.333_334).abs() < 0.001);
        assert_eq!(window.window_seconds, None);
        assert_eq!(window.reset_at, DateTime::from_timestamp(1_791_000_000, 0));
        assert_eq!(snapshot.identity.plan.as_deref(), Some("Creator"));
        assert_eq!(
            snapshot.provider_cost,
            Some(ProviderCost {
                used: 8_200.0,
                limit: Some(30_000.0),
                units: "credits".to_string(),
            })
        );
    }

    #[test]
    fn zero_limit_reports_zero_percent() {
        let snapshot = parse(
            include_str!("../../../fixtures/elevenlabs/zero_limit.json"),
            now(),
        )
        .expect("fixture parses");

        assert_eq!(snapshot.windows[0].used_percent, 0.0);
        assert_eq!(snapshot.identity.plan.as_deref(), Some("Free"));
    }

    #[test]
    fn missing_reset_leaves_reset_at_empty() {
        let snapshot = parse(
            include_str!("../../../fixtures/elevenlabs/no_reset.json"),
            now(),
        )
        .expect("fixture parses");

        assert_eq!(snapshot.windows[0].reset_at, None);
        assert!((snapshot.windows[0].used_percent - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn usage_over_limit_is_clamped() {
        let body = r#"{"tier":"starter","character_count":120,"character_limit":100}"#;
        let snapshot = parse(body, now()).expect("parses");
        assert_eq!(snapshot.windows[0].used_percent, 100.0);
    }

    fn snapshot_with(used: u64, limit: u64) -> UsageSnapshot {
        let body =
            format!(r#"{{"tier":"starter","character_count":{used},"character_limit":{limit}}}"#);
        parse(&body, now()).expect("parses")
    }

    #[test]
    fn key_usage_sums_series_and_sorts_largest_first() {
        let mut snapshot = snapshot_with(13_903, 30_010);
        apply_key_usage(
            &mut snapshot,
            r#"{"time":[1,2,3],"usage":{"Sam-LifeOS":[973.0,583.0,211.0],"LifeOS":[12000.0,0.0,163.0],"Idle":[0.0,0.0,0.0]}}"#,
        );
        assert_eq!(
            snapshot.credits_by_key,
            vec![
                KeyCreditUsage {
                    key_name: Some("LifeOS".to_string()),
                    credits: 12_163,
                },
                KeyCreditUsage {
                    key_name: Some("Sam-LifeOS".to_string()),
                    credits: 1_767,
                },
            ]
        );
    }

    #[test]
    fn usage_outside_keys_becomes_a_no_key_row() {
        let mut snapshot = snapshot_with(10_000, 30_000);
        apply_key_usage(&mut snapshot, r#"{"usage":{"LifeOS":[6000.0]}}"#);
        assert_eq!(
            snapshot.credits_by_key.last(),
            Some(&KeyCreditUsage {
                key_name: None,
                credits: 4_000,
            })
        );
    }

    #[test]
    fn small_counter_drift_is_not_a_no_key_row() {
        let mut snapshot = snapshot_with(10_050, 30_000);
        apply_key_usage(&mut snapshot, r#"{"usage":{"LifeOS":[10000.0]}}"#);
        assert_eq!(snapshot.credits_by_key.len(), 1);
    }

    #[test]
    fn unreadable_key_usage_leaves_snapshot_alone() {
        let mut snapshot = snapshot_with(100, 1_000);
        apply_key_usage(&mut snapshot, "not json");
        assert!(snapshot.credits_by_key.is_empty());
    }

    #[test]
    fn period_start_steps_back_one_refresh_period() {
        // 2026-11-13T17:25:39Z, the reset seen on a real account.
        let monthly = r#"{"tier":"starter","character_count":0,"character_limit":1,"next_character_count_reset_unix":1794590739,"character_refresh_period":"monthly_period"}"#;
        assert_eq!(
            period_start(monthly).map(|start| start.to_rfc3339()),
            Some("2026-10-13T17:25:39+00:00".to_string())
        );
        let annual = monthly.replace("monthly_period", "annual_period");
        assert_eq!(
            period_start(&annual).map(|start| start.to_rfc3339()),
            Some("2025-11-13T17:25:39+00:00".to_string())
        );
        assert_eq!(
            period_start(r#"{"tier":"free","character_count":0,"character_limit":1}"#),
            None
        );
    }

    #[test]
    fn invalid_body_is_a_decode_error() {
        assert!(matches!(
            parse("{}", now()),
            Err(ElevenLabsError::DecodeUsage(_))
        ));
    }
}
