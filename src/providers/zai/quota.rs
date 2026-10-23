// SPDX-License-Identifier: MPL-2.0

use crate::error::ZaiError;
use crate::model::{ProviderId, ProviderIdentity, UsageHeadline, UsageSnapshot, UsageWindow};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct ZaiQuotaResponse {
    code: Option<i64>,
    success: Option<bool>,
    data: Option<ZaiQuotaData>,
}

#[derive(Debug, Deserialize)]
struct ZaiQuotaData {
    #[serde(default, alias = "limit")]
    limits: Option<Vec<ZaiLimit>>,
    level: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ZaiLimit {
    #[serde(rename = "type")]
    limit_type: Option<String>,
    unit: Option<Value>,
    number: Option<Value>,
    usage: Option<Value>,
    #[serde(rename = "currentValue")]
    current_value: Option<Value>,
    remaining: Option<Value>,
    percentage: Option<Value>,
    #[serde(rename = "nextResetTime")]
    next_reset_time: Option<Value>,
    #[serde(rename = "startTime")]
    start_time: Option<Value>,
    #[serde(rename = "endTime")]
    end_time: Option<Value>,
}

#[derive(Clone, Copy)]
enum ZaiWindowSlot {
    FiveHour,
    Weekly,
    Mcp,
}

impl ZaiWindowSlot {
    const fn index(self) -> usize {
        match self {
            Self::FiveHour => 0,
            Self::Weekly => 1,
            Self::Mcp => 2,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::FiveHour => "5 Hour",
            Self::Weekly => "Weekly",
            Self::Mcp => "MCP",
        }
    }

    const fn window_seconds(self) -> Option<i64> {
        match self {
            Self::FiveHour => Some(5 * 3600),
            Self::Weekly => Some(7 * 24 * 3600),
            Self::Mcp => None,
        }
    }

    fn reset_description(self) -> Option<String> {
        match self {
            Self::FiveHour => Some("Resets every 5 hours".to_string()),
            Self::Weekly => Some("Resets weekly".to_string()),
            Self::Mcp => None,
        }
    }
}

pub fn parse(body: &str, updated_at: DateTime<Utc>) -> Result<UsageSnapshot, ZaiError> {
    let response: ZaiQuotaResponse = serde_json::from_str(body).map_err(ZaiError::DecodeUsage)?;
    if response.code != Some(200) || response.success != Some(true) {
        return Err(ZaiError::InvalidEnvelope);
    }
    let data = response.data.ok_or(ZaiError::InvalidEnvelope)?;
    let mut slots: [Option<UsageWindow>; 3] = std::array::from_fn(|_| None);

    for limit in data.limits.unwrap_or_default() {
        let Some(slot) = classify(&limit) else {
            continue;
        };
        let Some(used_percent) = used_percent(&limit) else {
            continue;
        };
        let window = UsageWindow {
            label: slot.label().to_string(),
            used_percent,
            reset_at: reset_at(&limit),
            window_seconds: slot
                .window_seconds()
                .or_else(|| measured_window_seconds(&limit)),
            reset_description: slot.reset_description(),
            group: None,
        };
        let index = slot.index();
        if slots[index].is_none() {
            slots[index] = Some(window);
        }
    }

    let windows: Vec<UsageWindow> = slots.into_iter().flatten().collect();
    if windows.is_empty() {
        return Err(ZaiError::NoUsageData);
    }

    Ok(UsageSnapshot {
        provider: ProviderId::Zai,
        source: "API Key".to_string(),
        updated_at,
        headline: UsageHeadline(0),
        windows,
        provider_cost: None,
        extra_usage: None,
        identity: ProviderIdentity {
            email: None,
            account_id: None,
            plan: normalized_plan(data.level.as_deref()),
            display_name: None,
        },
        tokens_by_day: Vec::new(),
        tokens_by_model: Vec::new(),
        credits_by_key: Vec::new(),
    })
}

fn classify(limit: &ZaiLimit) -> Option<ZaiWindowSlot> {
    let limit_type = limit.limit_type.as_deref()?;
    let unit = integer(limit.unit.as_ref())?;
    let number = integer(limit.number.as_ref())?;
    match (limit_type, unit, number) {
        ("CREDIT_LIMIT" | "TOKENS_LIMIT", 3, 5) => Some(ZaiWindowSlot::FiveHour),
        ("CREDIT_LIMIT" | "TOKENS_LIMIT", 6, 1) => Some(ZaiWindowSlot::Weekly),
        ("TIME_LIMIT", 5, 1) => Some(ZaiWindowSlot::Mcp),
        _ => None,
    }
}

fn used_percent(limit: &ZaiLimit) -> Option<f32> {
    if let Some(percentage) = finite_number(limit.percentage.as_ref()) {
        return Some((percentage as f32).clamp(0.0, 100.0));
    }
    let usage = finite_number(limit.usage.as_ref()).filter(|usage| *usage > 0.0)?;
    let numerator = finite_number(limit.current_value.as_ref())
        .or_else(|| finite_number(limit.remaining.as_ref()).map(|remaining| usage - remaining))?;
    let percent = numerator / usage * 100.0;
    percent
        .is_finite()
        .then_some((percent as f32).clamp(0.0, 100.0))
}

fn reset_at(limit: &ZaiLimit) -> Option<DateTime<Utc>> {
    integer(limit.next_reset_time.as_ref()).and_then(DateTime::from_timestamp_millis)
}

fn measured_window_seconds(limit: &ZaiLimit) -> Option<i64> {
    let start = integer(limit.start_time.as_ref())?;
    let end = integer(limit.end_time.as_ref())?;
    let millis = end.checked_sub(start)?;
    (millis > 0)
        .then_some(millis / 1000)
        .filter(|seconds| *seconds > 0)
}

fn integer(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(value) => value.as_i64(),
        Value::String(value) => value.trim().parse().ok(),
        _ => None,
    }
}

fn finite_number(value: Option<&Value>) -> Option<f64> {
    let number = match value? {
        Value::Number(value) => value.as_f64()?,
        Value::String(value) => value.trim().parse().ok()?,
        _ => return None,
    };
    number.is_finite().then_some(number)
}

fn normalized_plan(level: Option<&str>) -> Option<String> {
    let level = level?.trim();
    if level.is_empty() {
        return None;
    }
    Some(match level.to_ascii_lowercase().as_str() {
        "lite" => "Lite".to_string(),
        "pro" => "Pro".to_string(),
        "max" => "Max".to_string(),
        _ => level.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const UPDATED_AT: &str = "2026-08-04T06:21:48Z";

    fn updated_at() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(UPDATED_AT)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn parses_live_credit_fixture_in_canonical_order() {
        let snapshot = parse(
            include_str!("../../../fixtures/zai/credit_limit.json"),
            updated_at(),
        )
        .unwrap();

        assert_eq!(snapshot.provider, ProviderId::Zai);
        assert_eq!(snapshot.source, "API Key");
        assert_eq!(snapshot.identity.plan.as_deref(), Some("Pro"));
        assert_eq!(snapshot.windows.len(), 2);
        assert_eq!(snapshot.windows[0].label, "5 Hour");
        assert_eq!(snapshot.windows[0].used_percent, 0.0);
        assert_eq!(snapshot.windows[0].window_seconds, Some(5 * 3600));
        assert_eq!(snapshot.windows[1].label, "Weekly");
        assert_eq!(snapshot.windows[1].used_percent, 100.0);
        assert_eq!(snapshot.windows[1].window_seconds, Some(7 * 24 * 3600));
        assert!(snapshot.windows[1].reset_at.is_some());
    }

    #[test]
    fn parses_token_and_mcp_fixture_without_inventing_mcp_duration() {
        let snapshot = parse(
            include_str!("../../../fixtures/zai/token_mcp.json"),
            updated_at(),
        )
        .unwrap();

        assert_eq!(snapshot.windows.len(), 3);
        assert_eq!(
            snapshot
                .windows
                .iter()
                .map(|window| window.label.as_str())
                .collect::<Vec<_>>(),
            ["5 Hour", "Weekly", "MCP"]
        );
        assert_eq!(snapshot.windows[2].window_seconds, None);
        assert_eq!(snapshot.identity.plan.as_deref(), Some("Max"));
    }

    #[test]
    fn parses_mcp_only_fixture_as_successful_usage() {
        let snapshot = parse(
            include_str!("../../../fixtures/zai/mcp_only.json"),
            updated_at(),
        )
        .unwrap();

        assert_eq!(snapshot.windows.len(), 1);
        assert_eq!(snapshot.windows[0].label, "MCP");
        assert_eq!(snapshot.windows[0].used_percent, 25.0);
        assert_eq!(snapshot.windows[0].window_seconds, None);
    }

    #[test]
    fn input_order_does_not_change_window_order() {
        let body = r#"{
            "code":200,
            "success":true,
            "data":{"level":"custom","limits":[
                {"type":"TIME_LIMIT","unit":5,"number":1,"percentage":20},
                {"type":"CREDIT_LIMIT","unit":6,"number":1,"percentage":40},
                {"type":"CREDIT_LIMIT","unit":3,"number":5,"percentage":10}
            ]}
        }"#;

        let snapshot = parse(body, updated_at()).unwrap();

        assert_eq!(
            snapshot
                .windows
                .iter()
                .map(|window| window.label.as_str())
                .collect::<Vec<_>>(),
            ["5 Hour", "Weekly", "MCP"]
        );
        assert_eq!(snapshot.identity.plan.as_deref(), Some("custom"));
    }

    #[test]
    fn derives_and_clamps_usage_percent_without_creating_invalid_rows() {
        let body = r#"{
            "code":200,
            "success":true,
            "data":{"limits":[
                {"type":"CREDIT_LIMIT","unit":3,"number":5,"usage":100,"currentValue":150},
                {"type":"CREDIT_LIMIT","unit":6,"number":1,"usage":100,"remaining":25},
                {"type":"TIME_LIMIT","unit":5,"number":1,"usage":0,"remaining":0},
                {"type":"CREDIT_LIMIT","unit":99,"number":1,"percentage":0}
            ]}
        }"#;

        let snapshot = parse(body, updated_at()).unwrap();

        assert_eq!(snapshot.windows.len(), 2);
        assert_eq!(snapshot.windows[0].used_percent, 100.0);
        assert_eq!(snapshot.windows[1].used_percent, 75.0);
    }

    #[test]
    fn rejects_unsuccessful_empty_and_invalid_envelopes() {
        for body in [
            r#"{"code":500,"success":false,"data":{"limits":[]}}"#,
            r#"{"code":200,"success":true}"#,
            r#"{"code":200,"success":true,"data":{"limits":[]}}"#,
            r#"{"code":200,"success":true,"data":{"limits":[{"type":"UNKNOWN","unit":3,"number":5,"percentage":0}]}}"#,
        ] {
            assert!(parse(body, updated_at()).is_err());
        }
    }

    #[test]
    fn accepts_reported_mcp_start_and_end_span() {
        let body = r#"{
            "code":200,
            "success":true,
            "data":{"limits":[
                {"type":"TIME_LIMIT","unit":5,"number":1,"percentage":20,"startTime":1000,"endTime":86401000}
            ]}
        }"#;

        let snapshot = parse(body, updated_at()).unwrap();

        assert_eq!(snapshot.windows[0].window_seconds, Some(86400));
    }
}
