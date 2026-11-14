// SPDX-License-Identifier: MPL-2.0

use crate::error::FishError;
use crate::model::{
    ProviderCost, ProviderId, ProviderIdentity, UsageHeadline, UsageSnapshot, UsageWindow,
};
use chrono::{DateTime, Utc};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

/// `GET /wallet/self/api-credit`. Unknown fields are ignored. Both amounts are
/// dollars sent as decimal strings, e.g. `"42.75"`.
#[derive(Debug, Deserialize)]
struct ApiCreditResponse {
    #[serde(deserialize_with = "decimal")]
    credit: f64,
    #[serde(deserialize_with = "decimal")]
    cumulative_top_up: f64,
}

/// Reads a decimal that arrives as a string (the documented shape) or a plain
/// number, and rejects anything that is not a finite value.
fn decimal<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Text(String),
        Number(f64),
    }

    let value = match Raw::deserialize(deserializer)? {
        Raw::Text(text) => text
            .trim()
            .parse::<f64>()
            .map_err(|_| D::Error::custom("amount is not a decimal number"))?,
        Raw::Number(number) => number,
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(D::Error::custom("amount is not a finite number"))
    }
}

/// The one window Fish Audio gives us: pay-as-you-go API credit spent out of
/// everything you have had to spend. There is no reset time. The total is the
/// larger of lifetime top-ups and the current balance, because free or promo
/// credit can push the balance past the top-ups; that way "left" is always the
/// real balance, never a clamped figure.
pub fn parse(body: &str, updated_at: DateTime<Utc>) -> Result<UsageSnapshot, FishError> {
    let response: ApiCreditResponse = serde_json::from_str(body).map_err(FishError::DecodeUsage)?;

    let credit = response.credit;
    let total = response.cumulative_top_up.max(credit).max(0.0);
    let used = (total - credit).max(0.0);
    let used_percent = if total <= 0.0 {
        if credit < 0.0 { 100.0 } else { 0.0 }
    } else {
        ((used / total) * 100.0).clamp(0.0, 100.0) as f32
    };

    Ok(UsageSnapshot {
        provider: ProviderId::Fish,
        source: "API Key".to_string(),
        updated_at,
        headline: UsageHeadline(0),
        windows: vec![UsageWindow {
            label: "API credit".to_string(),
            used_percent,
            reset_at: None,
            window_seconds: None,
            reset_description: None,
            group: None,
        }],
        provider_cost: Some(ProviderCost {
            used,
            limit: Some(total),
            units: "USD".to_string(),
        }),
        extra_usage: None,
        identity: ProviderIdentity {
            email: None,
            account_id: None,
            plan: None,
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
    fn parses_live_response_shape() {
        let snapshot = parse(
            include_str!("../../../fixtures/fish/api_credit.json"),
            now(),
        )
        .expect("fixture parses");

        assert_eq!(snapshot.provider, ProviderId::Fish);
        assert_eq!(snapshot.windows.len(), 1);
        let window = &snapshot.windows[0];
        assert_eq!(window.label, "API credit");
        // (50 - 42.75) / 50 = 14.5 %
        assert!((window.used_percent - 14.5).abs() < 0.001);
        assert_eq!(window.reset_at, None);
        assert_eq!(window.window_seconds, None);
        assert_eq!(snapshot.identity.plan, None);

        let cost = snapshot.provider_cost.expect("cost present");
        assert!((cost.used - 7.25).abs() < 1e-9);
        assert_eq!(cost.limit, Some(50.0));
        assert_eq!(cost.units, "USD");
    }

    #[test]
    fn accepts_plain_json_numbers_too() {
        let snapshot = parse(r#"{"credit":25,"cumulative_top_up":100.0}"#, now()).expect("parses");
        assert!((snapshot.windows[0].used_percent - 75.0).abs() < f32::EPSILON);
    }

    #[test]
    fn nothing_topped_up_reports_zero_percent() {
        for body in [
            r#"{"credit":"0","cumulative_top_up":"0"}"#,
            r#"{"credit":"3","cumulative_top_up":"-1"}"#,
        ] {
            let snapshot = parse(body, now()).expect("parses");
            assert_eq!(snapshot.windows[0].used_percent, 0.0, "{body}");
        }
    }

    #[test]
    fn credit_above_top_up_keeps_the_real_balance() {
        // Free or promo credit can push the balance past what was topped up.
        let snapshot = parse(r#"{"credit":"60","cumulative_top_up":"50"}"#, now()).expect("parses");
        assert_eq!(snapshot.windows[0].used_percent, 0.0);
        let cost = snapshot.provider_cost.expect("cost");
        assert_eq!(cost.limit.expect("limit") - cost.used, 60.0);
    }

    #[test]
    fn credit_with_nothing_topped_up_is_still_shown() {
        let snapshot = parse(r#"{"credit":"3","cumulative_top_up":"0"}"#, now()).expect("parses");
        let cost = snapshot.provider_cost.expect("cost");
        assert_eq!(cost.limit.expect("limit") - cost.used, 3.0);
    }

    #[test]
    fn fully_spent_credit_is_one_hundred_percent() {
        let snapshot = parse(r#"{"credit":"-5","cumulative_top_up":"50"}"#, now()).expect("parses");
        assert_eq!(snapshot.windows[0].used_percent, 100.0);
    }

    #[test]
    fn malformed_bodies_are_decode_errors() {
        for body in [
            "{}",
            "not json",
            r#"{"credit":"48.9"}"#,
            r#"{"cumulative_top_up":"50"}"#,
            r#"{"credit":null,"cumulative_top_up":"50"}"#,
            r#"{"credit":"abc","cumulative_top_up":"50"}"#,
            r#"{"credit":"","cumulative_top_up":"50"}"#,
            r#"{"credit":"NaN","cumulative_top_up":"50"}"#,
            r#"{"credit":"48.9","cumulative_top_up":"inf"}"#,
            r#"{"credit":true,"cumulative_top_up":"50"}"#,
        ] {
            assert!(
                matches!(parse(body, now()), Err(FishError::DecodeUsage(_))),
                "{body} should fail to decode"
            );
        }
    }
}
