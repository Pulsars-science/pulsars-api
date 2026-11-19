//! The subscription (`openapi.yaml`, tag `billing`), paid through Stripe
//! (`src/stripe.rs`).
//!
//! - `GET /v1/billing/plans`: the prices on offer, read from Stripe and
//!   kept an hour in Redis.
//! - `GET /v1/me/subscription`: whether the member is a subscriber, and
//!   their subscription as Stripe last described it.
//! - `POST /v1/billing/checkout`: a Checkout session for a verified member
//!   who asked for immediate access and waived the withdrawal period.
//! - `POST /v1/billing/portal`: Stripe's billing portal, where the member
//!   changes their card, reads their invoices and cancels in a few clicks.
//! - `POST /v1/billing/webhook`: Stripe's calls, authenticated by their
//!   signature. Every call about a subscription reads it again from Stripe,
//!   so the order and repetition of the calls do not matter.
//!
//! What the rest of the site reads is the `abonne` role: it follows the
//! subscriptions that give access (`active`, `trialing`). An admin can also
//! give it by hand; the webhook only takes it away from a member who has a
//! Stripe subscription and none that still gives access.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use chrono::{DateTime, Utc};
use fred::prelude::{Expiration, KeysInterface as _};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::auth::no_store;
use super::site::ok;
use crate::auth::member::{MaybeMember, Member, Verified};
use crate::config::StripeConfig;
use crate::error::{ApiError, ParameterError};
use crate::extract::Json;
use crate::state::AppState;
use crate::stripe::{self, Price, Stripe, Subscription};

/// The role the site reads.
pub const SUBSCRIBER_ROLE: &str = "abonne";
const PRICE_CACHE_SECS: i64 = 3600;

fn configured(state: &AppState) -> Result<&StripeConfig, ApiError> {
    state
        .config
        .stripe
        .as_ref()
        .ok_or_else(|| ApiError::ServiceUnavailable {
            reason: "not_configured",
            detail: "Subscriptions are not configured on this server.".to_owned(),
        })
}

/// A plan on offer.
#[derive(Debug, Serialize, Deserialize)]
struct Plan {
    /// `monthly` or `yearly`.
    plan: String,
    /// In cents.
    amount: i64,
    currency: String,
    /// `month` or `year`.
    interval: String,
}

async fn cached_price(state: &AppState, stripe: &Stripe<'_>, id: &str) -> Result<Price, ApiError> {
    let key = format!("billing:price:{id}");
    let cached: Option<String> = state.redis.get(&key).await.unwrap_or(None);
    if let Some(price) = cached
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .and_then(|v| {
            Some(Price {
                id: id.to_owned(),
                unit_amount: v.get("unit_amount")?.as_i64()?,
                currency: v.get("currency")?.as_str()?.to_owned(),
                interval: v.get("interval")?.as_str()?.to_owned(),
            })
        })
    {
        return Ok(price);
    }
    let price = stripe.price(id).await?;
    let raw = serde_json::json!({
        "unit_amount": price.unit_amount,
        "currency": price.currency,
        "interval": price.interval,
    })
    .to_string();
    // A cache that cannot be written only costs a call next time.
    let _: Result<(), _> = state
        .redis
        .set(
            &key,
            raw,
            Some(Expiration::EX(PRICE_CACHE_SECS)),
            None,
            false,
        )
        .await;
    Ok(price)
}

/// `GET /v1/billing/plans`
pub async fn plans(State(state): State<AppState>) -> Result<Response, ApiError> {
    let config = configured(&state)?;
    let stripe = Stripe::new(config)?;
    let mut plans = Vec::new();
    for (plan, id) in [
        ("monthly", Some(&config.price_monthly)),
        ("yearly", config.price_yearly.as_ref()),
    ] {
        if let Some(id) = id {
            let price = cached_price(&state, &stripe, id).await?;
            plans.push(Plan {
                plan: plan.to_owned(),
                amount: price.unit_amount,
                currency: price.currency,
                interval: price.interval,
            });
        }
    }
    Ok(ok(plans))
}

#[derive(Debug, Serialize)]
struct SubscriptionView {
    status: String,
    /// `monthly`, `yearly`, or `null` for a price no longer on offer.
    plan: Option<&'static str>,
    current_period_end: Option<DateTime<Utc>>,
    cancel_at_period_end: bool,
}

/// Status, price, end of the period, cancellation at its end.
type SubscriptionRow = (String, Option<String>, Option<DateTime<Utc>>, bool);

#[derive(Debug, Serialize)]
struct MySubscription {
    /// Whether the member has the `abonne` role: paid or offered.
    subscriber: bool,
    /// The most recent Stripe subscription, if any.
    subscription: Option<SubscriptionView>,
    /// Whether the billing portal can open (the member has paid once).
    can_manage: bool,
}

fn plan_of(config: Option<&StripeConfig>, price: Option<&str>) -> Option<&'static str> {
    let (config, price) = (config?, price?);
    if price == config.price_monthly {
        Some("monthly")
    } else if config.price_yearly.as_deref() == Some(price) {
        Some("yearly")
    } else {
        None
    }
}

async fn has_role(state: &AppState, user_id: i64, slug: &str) -> Result<bool, ApiError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM site.role_user ru JOIN site.roles r ON r.id = ru.role_id
                         WHERE ru.user_id = $1 AND r.slug = $2)",
    )
    .bind(user_id)
    .bind(slug)
    .fetch_one(&state.db)
    .await?)
}

async fn customer_of(state: &AppState, user_id: i64) -> Result<Option<String>, ApiError> {
    Ok(
        sqlx::query_scalar("SELECT customer_id FROM site.stripe_customers WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?,
    )
}

fn signed_in(member: MaybeMember) -> Result<Member, ApiError> {
    member.0.ok_or(ApiError::SessionInvalid)
}

/// `GET /v1/me/subscription`
pub async fn mine(
    State(state): State<AppState>,
    member: MaybeMember,
) -> Result<Response, ApiError> {
    let member = signed_in(member)?;
    let row: Option<SubscriptionRow> = sqlx::query_as(
        "SELECT status::text, price_id::text,
                current_period_end AT TIME ZONE 'UTC', cancel_at_period_end
           FROM site.subscriptions WHERE user_id = $1
          ORDER BY updated_at DESC, created_at DESC LIMIT 1",
    )
    .bind(member.id())
    .fetch_optional(&state.db)
    .await?;
    let config = state.config.stripe.as_ref();
    Ok(ok(MySubscription {
        subscriber: has_role(&state, member.id(), SUBSCRIBER_ROLE).await?,
        subscription: row.map(|(status, price, end, cancel)| SubscriptionView {
            status,
            plan: plan_of(config, price.as_deref()),
            current_period_end: end,
            cancel_at_period_end: cancel,
        }),
        can_manage: config.is_some() && customer_of(&state, member.id()).await?.is_some(),
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckoutBody {
    /// `monthly` or `yearly`.
    plan: String,
    /// The member asks for immediate access and waives the withdrawal
    /// period: must be `true`.
    waive_withdrawal: bool,
}

#[derive(Debug, Serialize)]
struct Redirect {
    url: String,
}

/// What Checkout shows above its button, in the member's language.
fn submit_message(locale: &str) -> &'static str {
    match locale {
        "en" => {
            "By subscribing, you ask for immediate access and waive your 14-day \
             right of withdrawal. You can cancel at any time from your profile."
        }
        "ru" => {
            "Оформляя подписку, ты просишь немедленный доступ и отказываешься от \
             права на отказ в течение 14 дней. Отменить подписку можно в любой \
             момент в профиле."
        }
        _ => {
            "En t’abonnant, tu demandes l’accès immédiat et tu renonces à ton \
             droit de rétractation de 14 jours. Tu peux résilier à tout moment \
             depuis ton profil."
        }
    }
}

/// `POST /v1/billing/checkout`
pub async fn checkout(
    State(state): State<AppState>,
    Verified(member): Verified,
    Json(body): Json<CheckoutBody>,
) -> Result<Response, ApiError> {
    let config = configured(&state)?;
    let mut errors = Vec::new();
    let price = match body.plan.as_str() {
        "monthly" => Some(config.price_monthly.clone()),
        "yearly" => match &config.price_yearly {
            Some(price) => Some(price.clone()),
            None => {
                errors.push(ParameterError::field(
                    "plan",
                    "unavailable",
                    "The yearly plan is not on offer.",
                ));
                None
            }
        },
        _ => {
            errors.push(ParameterError::field(
                "plan",
                "invalid",
                "must be monthly or yearly",
            ));
            None
        }
    };
    if !body.waive_withdrawal {
        errors.push(ParameterError::field(
            "waive_withdrawal",
            "required",
            "Immediate access needs the waiver of the withdrawal period.",
        ));
    }
    let Some(price) = price.filter(|_| errors.is_empty()) else {
        return Err(ApiError::InvalidFields(errors));
    };

    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM site.subscriptions
                         WHERE user_id = $1 AND status IN ('active', 'trialing', 'past_due'))",
    )
    .bind(member.id())
    .fetch_one(&state.db)
    .await?;
    if already {
        return Err(ApiError::Conflict {
            reason: "already_subscribed",
            detail: "This account already has a subscription: manage it from the billing portal."
                .to_owned(),
        });
    }

    let stripe = Stripe::new(config)?;
    let customer = match customer_of(&state, member.id()).await? {
        Some(customer) => customer,
        None => {
            let created = stripe
                .create_customer(member.id(), &member.account.email, &member.account.pseudo)
                .await?;
            // Two checkouts at once: the first customer wins, the other one
            // stays unused in Stripe.
            sqlx::query(
                "INSERT INTO site.stripe_customers (user_id, customer_id) VALUES ($1, $2)
                 ON CONFLICT (user_id) DO NOTHING",
            )
            .bind(member.id())
            .bind(&created)
            .execute(&state.db)
            .await?;
            customer_of(&state, member.id()).await?.unwrap_or(created)
        }
    };
    let locale = member
        .account
        .locale
        .as_deref()
        .filter(|l| matches!(*l, "fr" | "en" | "ru"))
        .unwrap_or("fr");
    let (session, url) = stripe
        .checkout_session(
            &customer,
            member.id(),
            &price,
            locale,
            submit_message(locale),
        )
        .await?;
    sqlx::query(
        "INSERT INTO site.checkout_sessions (id, user_id, price_id, withdrawal_waived_at)
         VALUES ($1, $2, $3, date_trunc('second', now() AT TIME ZONE 'UTC'))",
    )
    .bind(&session)
    .bind(member.id())
    .bind(&price)
    .execute(&state.db)
    .await?;
    tracing::info!(user_id = member.id(), session, "billing: checkout opened");
    Ok(ok(Redirect { url }))
}

/// `POST /v1/billing/portal`
pub async fn portal(
    State(state): State<AppState>,
    member: MaybeMember,
) -> Result<Response, ApiError> {
    let member = signed_in(member)?;
    let config = configured(&state)?;
    let customer = customer_of(&state, member.id())
        .await?
        .ok_or_else(|| ApiError::Conflict {
            reason: "no_customer",
            detail: "This account has never subscribed.".to_owned(),
        })?;
    let url = Stripe::new(config)?.portal_session(&customer).await?;
    Ok(ok(Redirect { url }))
}

/// Copies a subscription from Stripe and brings the member's role in line.
/// A subscription of a customer the API does not know is ignored.
pub async fn sync(state: &AppState, stripe: &Stripe<'_>, id: &str) -> Result<(), ApiError> {
    let subscription: Subscription = stripe.subscription(id).await?;
    let user_id: Option<i64> =
        sqlx::query_scalar("SELECT user_id FROM site.stripe_customers WHERE customer_id = $1")
            .bind(&subscription.customer)
            .fetch_optional(&state.db)
            .await?;
    let Some(user_id) = user_id else {
        tracing::warn!(
            subscription = id,
            "billing: subscription of an unknown customer"
        );
        return Ok(());
    };
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "INSERT INTO site.subscriptions
                (id, user_id, status, price_id, current_period_end, cancel_at_period_end)
         VALUES ($1, $2, $3, $4, $5 AT TIME ZONE 'UTC', $6)
         ON CONFLICT (id) DO UPDATE SET
                status = EXCLUDED.status,
                price_id = EXCLUDED.price_id,
                current_period_end = EXCLUDED.current_period_end,
                cancel_at_period_end = EXCLUDED.cancel_at_period_end,
                updated_at = date_trunc('second', now() AT TIME ZONE 'UTC')",
    )
    .bind(&subscription.id)
    .bind(user_id)
    .bind(&subscription.status)
    .bind(&subscription.price_id)
    .bind(subscription.current_period_end)
    .bind(subscription.cancel_at_period_end)
    .execute(&mut *tx)
    .await?;
    let access: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM site.subscriptions
                         WHERE user_id = $1 AND status IN ('active', 'trialing'))",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;
    if access {
        sqlx::query(
            "INSERT INTO site.role_user (role_id, user_id)
             SELECT id, $1 FROM site.roles WHERE slug = $2
             ON CONFLICT (role_id, user_id) DO NOTHING",
        )
        .bind(user_id)
        .bind(SUBSCRIBER_ROLE)
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query(
            "DELETE FROM site.role_user
              WHERE user_id = $1 AND role_id = (SELECT id FROM site.roles WHERE slug = $2)",
        )
        .bind(user_id)
        .bind(SUBSCRIBER_ROLE)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    tracing::info!(
        user_id,
        subscription = id,
        status = subscription.status,
        access,
        "billing: subscription synced"
    );
    Ok(())
}

fn refused(detail: &str) -> Response {
    no_store(
        StatusCode::BAD_REQUEST,
        &serde_json::json!({"received": false, "detail": detail}),
    )
}

/// `POST /v1/billing/webhook`
pub async fn webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let config = configured(&state)?;
    let Some(signature) = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
    else {
        return Ok(refused("missing Stripe-Signature"));
    };
    if let Err(error) = stripe::verify_signature(
        signature,
        &body,
        config.webhook_secret.expose(),
        Utc::now().timestamp(),
    ) {
        tracing::warn!(?error, "billing: webhook signature refused");
        return Ok(refused("invalid signature"));
    }
    let Ok(event) = serde_json::from_slice::<Value>(&body) else {
        return Ok(refused("not JSON"));
    };
    let kind = event.get("type").and_then(Value::as_str).unwrap_or("");
    let object = event.pointer("/data/object").unwrap_or(&Value::Null);
    let text = |key: &str| object.get(key).and_then(Value::as_str);

    let subscription = match kind {
        "checkout.session.completed" => {
            if let Some(session) = text("id") {
                sqlx::query(
                    "UPDATE site.checkout_sessions
                        SET completed_at = date_trunc('second', now() AT TIME ZONE 'UTC')
                      WHERE id = $1 AND completed_at IS NULL",
                )
                .bind(session)
                .execute(&state.db)
                .await?;
            }
            text("subscription")
        }
        "customer.subscription.created"
        | "customer.subscription.updated"
        | "customer.subscription.deleted"
        | "customer.subscription.paused"
        | "customer.subscription.resumed" => text("id"),
        "invoice.paid" | "invoice.payment_failed" => text("subscription"),
        _ => None,
    };
    if let Some(id) = subscription {
        // On failure Stripe calls again later: the 503 asks it to.
        sync(&state, &Stripe::new(config)?, id).await?;
    }
    Ok(no_store(
        StatusCode::OK,
        &serde_json::json!({"received": true}),
    ))
}
