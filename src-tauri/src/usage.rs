use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageData {
    pub session: Option<UsageBucket>,
    pub weekly_all: Option<UsageBucket>,
    pub weekly_sonnet: Option<UsageBucket>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageBucket {
    pub utilization: f64,
    pub resets_at: DateTime<Utc>,
}

impl UsageData {
    pub fn empty() -> Self {
        Self {
            session: None,
            weekly_all: None,
            weekly_sonnet: None,
            last_updated: Utc::now(),
        }
    }

    pub fn max_utilization(&self) -> f64 {
        [&self.session, &self.weekly_all, &self.weekly_sonnet]
            .iter()
            .filter_map(|b| b.as_ref())
            .map(|b| b.utilization)
            .fold(0.0_f64, f64::max)
    }
}

pub fn parse_usage_headers(headers: &reqwest::header::HeaderMap) -> UsageData {
    let session = parse_bucket(headers, "5h");
    let weekly_all = parse_bucket(headers, "7d");
    let weekly_sonnet = parse_bucket(headers, "7d-sonnet")
        .or_else(|| parse_bucket(headers, "7d_sonnet"));

    UsageData {
        session,
        weekly_all,
        weekly_sonnet,
        last_updated: Utc::now(),
    }
}

fn parse_bucket(headers: &reqwest::header::HeaderMap, window: &str) -> Option<UsageBucket> {
    let util_key = format!("anthropic-ratelimit-unified-{}-utilization", window);
    let reset_key = format!("anthropic-ratelimit-unified-{}-reset", window);

    let raw_util = headers
        .get(&util_key)?
        .to_str()
        .ok()?
        .parse::<f64>()
        .ok()?;
    // API returns 0.0–1.0, we store as 0–100
    let utilization = if raw_util <= 1.0 { raw_util * 100.0 } else { raw_util };

    let reset_ts = headers
        .get(&reset_key)?
        .to_str()
        .ok()?
        .parse::<i64>()
        .ok()?;

    let resets_at = DateTime::from_timestamp(reset_ts, 0)?;

    Some(UsageBucket {
        utilization,
        resets_at,
    })
}

/// Send a minimal messages.create call to Anthropic and read rate-limit headers.
pub async fn fetch_usage(api_key: &str) -> Result<UsageData, String> {
    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "model": "claude-haiku-4-5-20251001",
        "max_tokens": 1,
        "messages": [{"role": "user", "content": "."}]
    });

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    // Rate limit headers are present even on 429 responses
    let headers = response.headers().clone();
    let status = response.status();
    let _ = response.text().await;

    let data = parse_usage_headers(&headers);

    // If we got no data and it wasn't a success or 429, report the error
    if data.session.is_none() && data.weekly_all.is_none() && !status.is_success() && status.as_u16() != 429 {
        return Err(format!("API returned status {}", status));
    }

    Ok(data)
}
