use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum State {
    Pending,
    Up,
    Down,
    Paused,
    Maintenance,
    Unknown,
    Degraded,
}
impl State {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Up => "UP",
            Self::Down => "DOWN",
            Self::Paused => "PAUSED",
            Self::Maintenance => "MAINTENANCE",
            Self::Unknown => "UNKNOWN",
            Self::Degraded => "DEGRADED",
        }
    }
}
/// Threshold counters reset on the opposite observation; only confirmed states notify.
pub fn transition(
    previous: &str,
    ok: bool,
    failures: i32,
    successes: i32,
    fail_threshold: i32,
    recovery_threshold: i32,
) -> (&'static str, i32, i32) {
    if ok {
        let count = successes + 1;
        (
            if count >= recovery_threshold {
                "UP"
            } else if previous == "DOWN" {
                "DOWN"
            } else {
                "PENDING"
            },
            0,
            count.min(recovery_threshold),
        )
    } else {
        let count = failures + 1;
        (
            if count >= fail_threshold {
                "DOWN"
            } else if previous == "UP" {
                "UP"
            } else {
                "PENDING"
            },
            count.min(fail_threshold),
            0,
        )
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInput {
    pub name: String,
    pub kind: String,
    pub target: String,
    #[serde(default = "default_interval")]
    pub interval_seconds: i32,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: i32,
    #[serde(default = "default_threshold")]
    pub failure_threshold: i32,
    #[serde(default = "default_threshold")]
    pub recovery_threshold: i32,
    #[serde(default)]
    pub group_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub public: bool,
    pub server_id: Option<String>,
    #[serde(default)]
    pub config: serde_json::Value,
}
fn default_interval() -> i32 {
    30
}
fn default_timeout() -> i32 {
    10
}
fn default_threshold() -> i32 {
    2
}
impl MonitorInput {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.name.trim().is_empty()
            || self.name.len() > 120
            || self.target.len() > 2048
            || self.description.len() > 2000
            || self.group_name.len() > 120
            || self.tags.len() > 20
            || self.tags.iter().any(|s| s.len() > 60)
        {
            return Err("Invalid name, target, group or tags");
        }
        if ![
            "http", "json", "tcp", "dns", "tls", "push", "docker", "pm2", "host", "cpu", "memory",
            "disk", "redis", "smtp", "postgres",
        ]
        .contains(&self.kind.as_str())
        {
            return Err("Unsupported monitor type");
        }
        if !(20..=86400).contains(&self.interval_seconds)
            || !(1..=30).contains(&self.timeout_seconds)
            || self.timeout_seconds >= self.interval_seconds
            || !(1..=10).contains(&self.failure_threshold)
            || !(1..=10).contains(&self.recovery_threshold)
        {
            return Err("Invalid timing or threshold");
        }
        if !self.config.is_null() && !self.config.is_object() {
            return Err("Config must be an object");
        }
        if let Some(method) = self.config.get("method")
            && method != "GET"
            && method != "HEAD"
        {
            return Err("Method must be GET or HEAD");
        }
        if let Some(status) = self.config.get("status")
            && !status.as_u64().is_some_and(|n| (100..=599).contains(&n))
        {
            return Err("Expected status must be 100–599");
        }
        for key in ["keyword", "expected"] {
            if let Some(v) = self.config.get(key)
                && !v.as_str().is_some_and(|s| s.len() <= 4096)
            {
                return Err("Invalid keyword/expected value");
            }
        }
        if let Some(days) = self.config.get("warning_days")
            && !days.as_i64().is_some_and(|n| (0..=365).contains(&n))
        {
            return Err("Warning days must be 0–365");
        }
        if let Some(grace) = self.config.get("heartbeat_grace_seconds")
            && (self.kind != "push" || !grace.as_i64().is_some_and(|n| (0..=86400).contains(&n)))
        {
            return Err("Push heartbeat grace must be 0–86400 seconds");
        }
        if let Some(retries) = self.config.get("check_retries")
            && (!network_kind(&self.kind) || !retries.as_u64().is_some_and(|n| n <= 3))
        {
            return Err("Network check retries must be 0–3");
        }
        if let Some(redirects) = self.config.get("follow_redirects")
            && (!matches!(self.kind.as_str(), "http" | "json") || !redirects.is_boolean())
        {
            return Err("HTTP redirect setting must be a boolean");
        }
        if let Some(retention) = self.config.get("retention_days")
            && !retention.as_u64().is_some_and(|n| (7..=365).contains(&n))
        {
            return Err("Raw check retention must be 7–365 days");
        }
        if let Some(target) = self.config.get("slo_target_percent") {
            if !target.as_f64().is_some_and(|n| n.is_finite() && (90.0..=99.999).contains(&n))
                || !self.config.get("slo_window_days").and_then(serde_json::Value::as_i64).is_some_and(|n| [7,30,90].contains(&n))
                || !self.config.get("slo_alert_percent").and_then(serde_json::Value::as_i64).is_some_and(|n| (1..=100).contains(&n))
            {
                return Err("SLO target, window or alert threshold is invalid");
            }
        } else if self.config.get("slo_window_days").is_some() || self.config.get("slo_alert_percent").is_some() {
            return Err("SLO target is required when SLO options are set");
        }
        for key in ["cpu_threshold", "memory_threshold", "disk_threshold"] {
            if let Some(v) = self.config.get(key)
                && !v.as_f64().is_some_and(|n| (1.0..=100.0).contains(&n))
            {
                return Err("Resource thresholds must be 1–100");
            }
        }
        if let Some(v) = self.config.get("record_type")
            && !v.as_str().is_some_and(|v| {
                ["A", "AAAA", "CNAME", "MX", "TXT", "NS", "SRV", "IP"].contains(&v)
            })
        {
            return Err("Unsupported DNS record type");
        }
        if let Some(v) = self.config.get("json_pointer")
            && !v
                .as_str()
                .is_some_and(|v| v.len() <= 512 && (v.is_empty() || v.starts_with('/')))
        {
            return Err("Use a JSON Pointer, such as /status");
        }
        for key in ["json_component_id", "json_google_product_id"] {
            if let Some(id) = self.config.get(key)
                && (self.kind != "json"
                    || !id.as_str().is_some_and(|s| {
                        !s.is_empty()
                            && s.len() <= 128
                            && s.bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                    }))
            {
                return Err("Invalid status component ID");
            }
        }
        if self.config.get("json_google_product_id").is_some()
            && (self.target != "https://status.cloud.google.com/incidents.json"
                || self.config.get("json_component_id").is_some())
        {
            return Err("Google Cloud product checks require the official incident feed");
        }
        if self.kind == "json"
            && self.config.get("json_component_id").is_none()
            && self.config.get("json_google_product_id").is_none()
            && self.config.get("json_expected").is_none()
        {
            return Err("JSON monitors require an expected JSON value");
        }
        if self
            .config
            .get("json_expected")
            .is_some_and(|v| v.to_string().len() > 16384)
        {
            return Err("Expected JSON value is too large");
        }
        if let Some(v) = self.config.get("disk_mount")
            && !v.as_str().is_some_and(|s| s.len() <= 1024)
        {
            return Err("Invalid disk mount");
        }
        if let Some(v) = self.config.get("load_threshold")
            && !v.as_f64().is_some_and(|n| n > 0.0 && n <= 10000.0)
        {
            return Err("Invalid load threshold");
        }
        if ["docker", "pm2", "host", "cpu", "memory", "disk"].contains(&self.kind.as_str())
            && self.server_id.is_none()
        {
            return Err("This monitor requires an enrolled server");
        }
        Ok(())
    }
}
pub fn uptime(up: f64, down: f64) -> Option<f64> {
    if up + down > 0.0 {
        Some(up / (up + down) * 100.0)
    } else {
        None
    }
}
pub fn can_write(role: &str) -> bool {
    matches!(role, "owner" | "admin")
}
pub fn network_kind(kind: &str) -> bool {
    [
        "http", "json", "tcp", "dns", "tls", "redis", "smtp", "postgres",
    ]
    .contains(&kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hysteresis() {
        assert_eq!(transition("UP", false, 0, 2, 2, 2), ("UP", 1, 0));
        assert_eq!(transition("UP", false, 1, 0, 2, 2), ("DOWN", 2, 0));
        assert_eq!(transition("DOWN", true, 2, 0, 2, 2), ("DOWN", 0, 1));
        assert_eq!(transition("DOWN", true, 0, 1, 2, 2), ("UP", 0, 2));
    }
    #[test]
    fn unknown_is_not_uptime() {
        assert_eq!(uptime(0.0, 0.0), None);
        assert_eq!(uptime(90.0, 10.0), Some(90.0));
    }
    #[test]
    fn permissions() {
        assert!(!can_write("viewer"));
        assert!(!can_write("unknown"));
        assert!(can_write("owner"));
    }
    #[test]
    fn bounded_observation_options() {
        let mut monitor = MonitorInput {
            name: "probe".into(),
            kind: "http".into(),
            target: "https://example.com".into(),
            interval_seconds: 30,
            timeout_seconds: 10,
            failure_threshold: 2,
            recovery_threshold: 2,
            group_name: String::new(),
            description: String::new(),
            tags: vec![],
            public: false,
            server_id: None,
            config: serde_json::json!({"check_retries":3}),
        };
        assert!(monitor.validate().is_ok());
        monitor.config = serde_json::json!({"check_retries":4});
        assert!(monitor.validate().is_err());
        monitor.kind = "push".into();
        monitor.config = serde_json::json!({"heartbeat_grace_seconds":0});
        assert!(monitor.validate().is_ok());
        monitor.config = serde_json::json!({"heartbeat_grace_seconds":86401});
        assert!(monitor.validate().is_err());
        monitor.config = serde_json::json!({"retention_days":7});
        assert!(monitor.validate().is_ok());
        monitor.config = serde_json::json!({"retention_days":366});
        assert!(monitor.validate().is_err());
        monitor.config = serde_json::json!({"slo_target_percent":99.9,"slo_window_days":30,"slo_alert_percent":75});
        assert!(monitor.validate().is_ok());
        monitor.config = serde_json::json!({"slo_target_percent":100,"slo_window_days":30,"slo_alert_percent":75});
        assert!(monitor.validate().is_err());
        monitor.kind = "json".into();
        monitor.config = serde_json::json!({"json_component_id":"abc-123"});
        assert!(monitor.validate().is_ok());
        monitor.config = serde_json::json!({"json_component_id":"../other"});
        assert!(monitor.validate().is_err());
        monitor.target = "https://status.cloud.google.com/incidents.json".into();
        monitor.config = serde_json::json!({"json_google_product_id":"abc123"});
        assert!(monitor.validate().is_ok());
    }
}
