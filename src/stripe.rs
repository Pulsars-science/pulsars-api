//! Stripe, which takes the subscription payments (`config::StripeConfig`).
//!
//! The API speaks to Stripe's REST API directly (form-encoded bodies, JSON
//! answers) with the same HTTP client as DeepL and Umami: no SDK. It only
//! needs five calls: create a customer, open a Checkout session, open a
//! billing portal session, read a subscription, read a price.
//!
//! The webhook calls are authenticated by their `Stripe-Signature` header:
//! an HMAC-SHA256 of `{timestamp}.{body}` under the endpoint's secret
//! (`whsec_…`), checked in constant time, with five minutes of tolerance
//! against replays.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::config::StripeConfig;
use crate::error::ApiError;

/// The API version the answers are read with: a change on Stripe's side
/// never changes what this module receives.
pub const API_VERSION: &str = "2024-06-20";
const TIMEOUT: Duration = Duration::from_secs(10);
/// How old a signed webhook call may be.
pub const SIGNATURE_TOLERANCE_SECS: i64 = 300;

pub struct Stripe<'a> {
    config: &'a StripeConfig,
    http: reqwest::Client,
}

/// A subscription as the API keeps it (`site.subscriptions`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    pub id: String,
    pub customer: String,
    /// `active`, `trialing`, `past_due`, `canceled`, `unpaid`,
    /// `incomplete`, `incomplete_expired` or `paused`.
    pub status: String,
    pub price_id: Option<String>,
    pub current_period_end: Option<DateTime<Utc>>,
    pub cancel_at_period_end: bool,
}

impl Subscription {
    /// Whether the member gets the subscriber's access.
    pub fn grants_access(&self) -> bool {
        matches!(self.status.as_str(), "active" | "trialing")
    }
}

/// A price, for the subscription page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Price {
    pub id: String,
    /// In the currency's smallest unit (cents).
    pub unit_amount: i64,
    pub currency: String,
    /// `month` or `year`.
    pub interval: String,
}

fn upstream(detail: impl Into<String>) -> ApiError {
    ApiError::ServiceUnavailable {
        reason: "upstream_failed",
        detail: detail.into(),
    }
}

impl<'a> Stripe<'a> {
    pub fn new(config: &'a StripeConfig) -> Result<Self, ApiError> {
        let https_only = config.api_url.starts_with("https://");
        let http = crate::http_client::http_client(https_only, TIMEOUT, TIMEOUT)
            .map_err(|error| ApiError::Internal(format!("stripe: {error}")))?;
        Ok(Self { config, http })
    }

    async fn call(
        &self,
        method: reqwest::Method,
        path: &str,
        form: &[(&str, &str)],
    ) -> Result<Value, ApiError> {
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.config.api_url))
            .bearer_auth(self.config.secret_key.expose())
            .header("Stripe-Version", API_VERSION);
        if !form.is_empty() {
            let body = form_urlencoded::Serializer::new(String::new())
                .extend_pairs(form.iter().copied())
                .finish();
            request = request
                .header(
                    reqwest::header::CONTENT_TYPE,
                    "application/x-www-form-urlencoded",
                )
                .body(body);
        }
        let response = request.send().await.map_err(|error| {
            tracing::warn!(%error, path, "stripe: no answer");
            upstream("Stripe did not answer.")
        })?;
        let status = response.status();
        let bytes = response.bytes().await.map_err(|error| {
            tracing::warn!(%error, path, "stripe: unreadable answer");
            upstream("Stripe sent an unreadable answer.")
        })?;
        let body: Value = serde_json::from_slice(&bytes).map_err(|error| {
            tracing::warn!(%error, path, "stripe: answer is not JSON");
            upstream("Stripe sent an unreadable answer.")
        })?;
        if !status.is_success() {
            // Stripe's message names the problem without any secret.
            let message = body
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("");
            tracing::warn!(%status, path, message, "stripe: refused");
            return Err(upstream("Stripe refused the request."));
        }
        Ok(body)
    }

    fn id_of(body: &Value, what: &str) -> Result<String, ApiError> {
        body.get("id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| upstream(format!("Stripe sent a {what} without an id.")))
    }

    /// A new customer for a member; `metadata[user_id]` links it back.
    pub async fn create_customer(
        &self,
        user_id: i64,
        email: &str,
        pseudo: &str,
    ) -> Result<String, ApiError> {
        let user = user_id.to_string();
        let body = self
            .call(
                reqwest::Method::POST,
                "/v1/customers",
                &[
                    ("email", email),
                    ("name", pseudo),
                    ("metadata[user_id]", &user),
                ],
            )
            .await?;
        Self::id_of(&body, "customer")
    }

    /// A Checkout session for one subscription to `price`; its URL.
    pub async fn checkout_session(
        &self,
        customer: &str,
        user_id: i64,
        price: &str,
        locale: &str,
        submit_message: &str,
    ) -> Result<(String, String), ApiError> {
        let user = user_id.to_string();
        let success = format!(
            "{}/abonnement/merci?session_id={{CHECKOUT_SESSION_ID}}",
            self.config.return_url
        );
        let cancel = format!("{}/abonnement?annule=1", self.config.return_url);
        let body = self
            .call(
                reqwest::Method::POST,
                "/v1/checkout/sessions",
                &[
                    ("mode", "subscription"),
                    ("customer", customer),
                    ("client_reference_id", &user),
                    ("line_items[0][price]", price),
                    ("line_items[0][quantity]", "1"),
                    ("success_url", &success),
                    ("cancel_url", &cancel),
                    ("locale", locale),
                    ("allow_promotion_codes", "true"),
                    ("metadata[user_id]", &user),
                    ("subscription_data[metadata][user_id]", &user),
                    ("custom_text[submit][message]", submit_message),
                ],
            )
            .await?;
        let id = Self::id_of(&body, "Checkout session")?;
        let url = body
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| upstream("Stripe sent a Checkout session without a URL."))?;
        Ok((id, url.to_owned()))
    }

    /// A billing portal session (change of card, invoices, cancellation);
    /// its URL.
    pub async fn portal_session(&self, customer: &str) -> Result<String, ApiError> {
        let back = format!("{}/abonnement", self.config.return_url);
        let body = self
            .call(
                reqwest::Method::POST,
                "/v1/billing_portal/sessions",
                &[("customer", customer), ("return_url", &back)],
            )
            .await?;
        body.get("url")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| upstream("Stripe sent a portal session without a URL."))
    }

    /// The subscription as Stripe has it now.
    pub async fn subscription(&self, id: &str) -> Result<Subscription, ApiError> {
        let path = format!("/v1/subscriptions/{}", crate::links::encode(id, false));
        let body = self.call(reqwest::Method::GET, &path, &[]).await?;
        parse_subscription(&body).ok_or_else(|| upstream("Stripe sent an unreadable subscription."))
    }

    pub async fn price(&self, id: &str) -> Result<Price, ApiError> {
        let path = format!("/v1/prices/{}", crate::links::encode(id, false));
        let body = self.call(reqwest::Method::GET, &path, &[]).await?;
        parse_price(&body).ok_or_else(|| upstream("Stripe sent an unreadable price."))
    }
}

fn parse_price(body: &Value) -> Option<Price> {
    Some(Price {
        id: body.get("id")?.as_str()?.to_owned(),
        unit_amount: body.get("unit_amount")?.as_i64()?,
        currency: body.get("currency")?.as_str()?.to_owned(),
        interval: body.pointer("/recurring/interval")?.as_str()?.to_owned(),
    })
}

/// Reads a subscription object. `current_period_end` moved from the
/// subscription to its items in later API versions: both are read.
pub fn parse_subscription(body: &Value) -> Option<Subscription> {
    let item = body.pointer("/items/data/0");
    let period_end = body
        .get("current_period_end")
        .and_then(Value::as_i64)
        .or_else(|| item?.get("current_period_end")?.as_i64())
        .and_then(|secs| DateTime::from_timestamp(secs, 0));
    let customer = match body.get("customer")? {
        Value::String(id) => id.clone(),
        expanded => expanded.get("id")?.as_str()?.to_owned(),
    };
    Some(Subscription {
        id: body.get("id")?.as_str()?.to_owned(),
        customer,
        status: body.get("status")?.as_str()?.to_owned(),
        price_id: item
            .and_then(|i| i.pointer("/price/id"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        current_period_end: period_end,
        cancel_at_period_end: body
            .get("cancel_at_period_end")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// Why a webhook call is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureError {
    /// No `Stripe-Signature`, or one without `t=` or `v1=`.
    Malformed,
    /// Signed more than five minutes away from now.
    Expired,
    /// No `v1` signature matches.
    Mismatch,
}

/// Checks a `Stripe-Signature` header against the raw body.
pub fn verify_signature(
    header: &str,
    body: &[u8],
    secret: &str,
    now: i64,
) -> Result<(), SignatureError> {
    let mut timestamp = None;
    let mut signatures = Vec::new();
    for part in header.split(',') {
        match part.trim().split_once('=') {
            Some(("t", value)) => timestamp = value.parse::<i64>().ok(),
            Some(("v1", value)) => signatures.push(value),
            _ => {}
        }
    }
    let timestamp = timestamp.ok_or(SignatureError::Malformed)?;
    if signatures.is_empty() {
        return Err(SignatureError::Malformed);
    }
    if (now - timestamp).abs() > SIGNATURE_TOLERANCE_SECS {
        return Err(SignatureError::Expired);
    }
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret.as_bytes());
    let mut signed = timestamp.to_string().into_bytes();
    signed.push(b'.');
    signed.extend_from_slice(body);
    let matches = signatures.iter().any(|hex| {
        decode_hex(hex).is_some_and(|tag| ring::hmac::verify(&key, &signed, &tag).is_ok())
    });
    if matches {
        Ok(())
    } else {
        Err(SignatureError::Mismatch)
    }
}

/// The `Stripe-Signature` header Stripe would send: for the tests.
pub fn sign(body: &[u8], secret: &str, timestamp: i64) -> String {
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret.as_bytes());
    let mut signed = timestamp.to_string().into_bytes();
    signed.push(b'.');
    signed.extend_from_slice(body);
    let tag = ring::hmac::sign(&key, &signed);
    let hex: String = tag.as_ref().iter().map(|b| format!("{b:02x}")).collect();
    format!("t={timestamp},v1={hex}")
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(value.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "whsec_test_secret";

    #[test]
    fn a_signed_body_is_accepted() {
        let body = br#"{"id":"evt_1"}"#;
        let header = sign(body, SECRET, 1_700_000_000);
        assert_eq!(
            verify_signature(&header, body, SECRET, 1_700_000_100),
            Ok(())
        );
    }

    #[test]
    fn a_changed_body_or_secret_is_refused() {
        let body = br#"{"id":"evt_1"}"#;
        let header = sign(body, SECRET, 1_700_000_000);
        assert_eq!(
            verify_signature(&header, br#"{"id":"evt_2"}"#, SECRET, 1_700_000_000),
            Err(SignatureError::Mismatch)
        );
        assert_eq!(
            verify_signature(&header, body, "whsec_other", 1_700_000_000),
            Err(SignatureError::Mismatch)
        );
    }

    #[test]
    fn an_old_or_malformed_signature_is_refused() {
        let body = b"{}";
        let header = sign(body, SECRET, 1_700_000_000);
        assert_eq!(
            verify_signature(&header, body, SECRET, 1_700_000_000 + 301),
            Err(SignatureError::Expired)
        );
        assert_eq!(
            verify_signature("v1=abcd", body, SECRET, 0),
            Err(SignatureError::Malformed)
        );
        assert_eq!(
            verify_signature("t=0,v1=zz", body, SECRET, 0),
            Err(SignatureError::Mismatch)
        );
    }

    #[test]
    fn both_places_of_the_period_end_are_read() {
        let old = serde_json::json!({
            "id": "sub_1", "customer": "cus_1", "status": "active",
            "current_period_end": 1_800_000_000, "cancel_at_period_end": true,
            "items": {"data": [{"price": {"id": "price_m"}}]}
        });
        let new = serde_json::json!({
            "id": "sub_1", "customer": {"id": "cus_1"}, "status": "canceled",
            "items": {"data": [{"price": {"id": "price_m"}, "current_period_end": 1_800_000_000}]}
        });
        let old = parse_subscription(&old).unwrap();
        let new = parse_subscription(&new).unwrap();
        assert_eq!(old.current_period_end, new.current_period_end);
        assert_eq!(new.customer, "cus_1");
        assert!(old.grants_access() && old.cancel_at_period_end);
        assert!(!new.grants_access());
        assert_eq!(old.price_id.as_deref(), Some("price_m"));
    }
}
