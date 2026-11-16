use crate::*;
use aegis_core::MonitorInput;
use serde::{Deserialize, Serialize};
#[derive(Deserialize)]
pub struct MonitorRequest {
    #[serde(flatten)]
    pub monitor: MonitorInput,
    pub credentials: Option<HttpCredentials>,
    pub monitor_id: Option<uuid::Uuid>,
}
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct HttpCredentials {
    pub clear: bool,
    pub bearer: String,
    pub basic_username: String,
    pub basic_password: String,
    pub headers: Vec<SecretHeader>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretHeader {
    pub name: String,
    pub value: String,
}
impl HttpCredentials {
    pub fn validate(&self, kind: &str) -> Result<()> {
        if !["http", "json"].contains(&kind) {
            return Err(bad(
                "Credentials are supported only for HTTP and JSON monitors",
            ));
        }
        if self.headers.len() > 20
            || self.bearer.len() > 8192
            || self.basic_username.len() > 256
            || self.basic_password.len() > 2048
            || (!self.bearer.is_empty()
                && (!self.basic_username.is_empty() || !self.basic_password.is_empty()))
            || json!(self).to_string().len() > 16384
        {
            return Err(bad("Invalid authentication mode or credential size"));
        }
        if self.clear
            && (!self.bearer.is_empty()
                || !self.basic_username.is_empty()
                || !self.basic_password.is_empty()
                || !self.headers.is_empty())
        {
            return Err(bad(
                "Clear credentials cannot be combined with new credentials",
            ));
        }
        for h in &self.headers {
            let name = reqwest::header::HeaderName::from_bytes(h.name.as_bytes())
                .map_err(|_| bad("Invalid header name"))?;
            if [
                "host",
                "content-length",
                "connection",
                "transfer-encoding",
                "cookie",
                "authorization",
                "proxy-authorization",
                "proxy-connection",
                "te",
                "upgrade",
            ]
            .contains(&name.as_str())
                || h.value.len() > 4096
                || reqwest::header::HeaderValue::from_str(&h.value).is_err()
            {
                return Err(bad("Header is forbidden, too long or invalid"));
            }
        }
        if !self.bearer.is_empty()
            && reqwest::header::HeaderValue::from_str(&format!("Bearer {}", self.bearer)).is_err()
        {
            return Err(bad("Invalid bearer token"));
        }
        Ok(())
    }
}
pub fn protected(a: &App, m: &MonitorInput, c: Option<&HttpCredentials>) -> Result<Option<String>> {
    if let Some(c) = c {
        c.validate(&m.kind)?;
        if c.clear {
            return Ok(None);
        }
        Ok(Some(encrypt(a, &json!(c).to_string())?))
    } else {
        Ok(None)
    }
}
pub fn inject(a: &App, m: &mut MonitorInput, stored: Option<String>) -> Result<()> {
    if let Some(stored) = stored {
        let c: HttpCredentials = serde_json::from_str(&decrypt(a, &stored)?)
            .map_err(|_| bad("Stored credentials are invalid"))?;
        c.validate(&m.kind)?;
        if !m.config.is_object() {
            m.config = json!({});
        }
        m.config["_credentials"] = json!(c);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forbids_header_smuggling() {
        let c = HttpCredentials {
            headers: vec![SecretHeader {
                name: "Host".into(),
                value: "metadata.internal".into(),
            }],
            ..Default::default()
        };
        assert!(c.validate("http").is_err());
        let c = HttpCredentials {
            bearer: "token\r\nCookie: secret".into(),
            ..Default::default()
        };
        assert!(c.validate("http").is_err());
    }
}
