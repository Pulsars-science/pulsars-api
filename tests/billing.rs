//! The subscription: plans, checkout, portal and Stripe's webhook, against
//! a fake Stripe served on a local port.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use common::{Call, Env, TestMember};
use serde_json::{Value, json};

const WEBHOOK_SECRET: &str = "whsec_test_pulsars";

/// What the fake Stripe received and holds.
#[derive(Default)]
struct Fake {
    customers: usize,
    checkouts: Vec<HashMap<String, String>>,
    subscriptions: HashMap<String, Value>,
}

type Shared = Arc<Mutex<Fake>>;

fn form(body: &str) -> HashMap<String, String> {
    form_urlencoded::parse(body.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

/// Starts the fake Stripe; its base URL.
async fn fake_stripe(shared: Shared) -> String {
    let app = Router::new()
        .route(
            "/v1/customers",
            post(|State(s): State<Shared>, _body: String| async move {
                let mut fake = s.lock().unwrap();
                fake.customers += 1;
                axum::Json(
                    json!({"id": format!("cus_test_{}_{}", fake.customers, common::unique())}),
                )
            }),
        )
        .route(
            "/v1/checkout/sessions",
            post(|State(s): State<Shared>, body: String| async move {
                let mut fake = s.lock().unwrap();
                fake.checkouts.push(form(&body));
                let id = format!("cs_test_{}_{}", fake.checkouts.len(), common::unique());
                axum::Json(json!({"id": id, "url": format!("https://checkout.stripe.test/{id}")}))
            }),
        )
        .route(
            "/v1/billing_portal/sessions",
            post(|body: String| async move {
                let customer = form(&body).remove("customer").unwrap_or_default();
                axum::Json(json!({"url": format!("https://billing.stripe.test/{customer}")}))
            }),
        )
        .route(
            "/v1/subscriptions/{id}",
            get(
                |State(s): State<Shared>, Path(id): Path<String>| async move {
                    match s.lock().unwrap().subscriptions.get(&id) {
                        Some(sub) => (StatusCode::OK, axum::Json(sub.clone())),
                        None => (
                            StatusCode::NOT_FOUND,
                            axum::Json(json!({"error": {"message": "No such subscription"}})),
                        ),
                    }
                },
            ),
        )
        .route(
            "/v1/prices/{id}",
            get(|Path(id): Path<String>| async move {
                let (amount, interval) = if id == "price_year" {
                    (3900, "year")
                } else {
                    (399, "month")
                };
                axum::Json(json!({
                    "id": id, "unit_amount": amount, "currency": "eur",
                    "recurring": {"interval": interval}
                }))
            }),
        )
        .with_state(shared);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await });
    url
}

async fn env_with_stripe() -> Option<(Env, Shared)> {
    let shared = Shared::default();
    let url = fake_stripe(shared.clone()).await;
    let env = Env::with(&[
        ("STRIPE_SECRET_KEY", "sk_test_pulsars"),
        ("STRIPE_WEBHOOK_SECRET", WEBHOOK_SECRET),
        ("STRIPE_PRICE_MONTHLY", "price_month"),
        ("STRIPE_PRICE_YEARLY", "price_year"),
        ("STRIPE_API_URL", url.as_str()),
        ("STRIPE_RETURN_URL", "http://localhost:3000"),
    ])
    .await?;
    Some((env, shared))
}

async fn webhook(env: &Env, event: Value) -> (StatusCode, Value) {
    let body = event.to_string();
    let signature = pulsars_api::stripe::sign(
        body.as_bytes(),
        WEBHOOK_SECRET,
        chrono::Utc::now().timestamp(),
    );
    Call::post("/v1/billing/webhook")
        .json(event)
        .header("stripe-signature", &signature)
        .send(&env.app)
        .await
}

async fn customer_of(env: &Env, member: &TestMember) -> String {
    sqlx::query_scalar("SELECT customer_id FROM site.stripe_customers WHERE user_id = $1")
        .bind(member.id)
        .fetch_one(&env.owner)
        .await
        .unwrap()
}

fn subscription(id: &str, customer: &str, status: &str, price: &str) -> Value {
    json!({
        "id": id, "object": "subscription", "customer": customer, "status": status,
        "cancel_at_period_end": false, "current_period_end": 1_900_000_000,
        "items": {"data": [{"price": {"id": price}}]}
    })
}

async fn subscribe(env: &Env, member: &TestMember) -> (StatusCode, Value) {
    Call::post("/v1/billing/checkout")
        .member(member)
        .json(json!({"plan": "monthly", "waive_withdrawal": true}))
        .send(&env.app)
        .await
}

#[tokio::test]
async fn without_stripe_the_routes_say_not_configured() {
    let Some(env) = Env::new().await else { return };
    let (status, body) = Call::get("/v1/billing/plans").send(&env.app).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    let (status, _) = webhook(&env, json!({"type": "ping"})).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn the_plans_come_from_stripe() {
    let Some((env, _)) = env_with_stripe().await else {
        return;
    };
    let (status, body) = Call::get("/v1/billing/plans").send(&env.app).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"][0]["plan"], "monthly");
    assert_eq!(body["data"][0]["amount"], 399);
    assert_eq!(body["data"][1]["interval"], "year");
}

#[tokio::test]
async fn checkout_needs_a_verified_member_and_the_waiver() {
    let Some((env, shared)) = env_with_stripe().await else {
        return;
    };
    let unverified = env.member().create().await;
    let (status, _) = subscribe(&env, &unverified).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let member = env.member().verified().create().await;
    let (status, body) = Call::post("/v1/billing/checkout")
        .member(&member)
        .json(json!({"plan": "monthly", "waive_withdrawal": false}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(
        common::codes(&body),
        [("waive_withdrawal".to_owned(), "required".to_owned())]
    );

    let (status, body) = subscribe(&env, &member).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["data"]["url"]
            .as_str()
            .unwrap()
            .starts_with("https://checkout.stripe.test/")
    );

    let sent = shared.lock().unwrap().checkouts.last().cloned().unwrap();
    assert_eq!(sent["mode"], "subscription");
    assert_eq!(sent["line_items[0][price]"], "price_month");
    assert_eq!(sent["client_reference_id"], member.id.to_string());
    assert!(sent["success_url"].starts_with("http://localhost:3000/abonnement/merci"));

    let waived: bool = sqlx::query_scalar(
        "SELECT withdrawal_waived_at IS NOT NULL FROM site.checkout_sessions WHERE user_id = $1",
    )
    .bind(member.id)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    assert!(waived);

    // A second checkout reuses the same Stripe customer.
    let customers = shared.lock().unwrap().customers;
    let (status, _) = subscribe(&env, &member).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(shared.lock().unwrap().customers, customers);
}

#[tokio::test]
async fn the_webhook_needs_stripes_signature() {
    let Some((env, _)) = env_with_stripe().await else {
        return;
    };
    let (status, _) = Call::post("/v1/billing/webhook")
        .json(json!({"type": "customer.subscription.updated"}))
        .header("stripe-signature", "t=1,v1=00")
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = Call::post("/v1/billing/webhook")
        .json(json!({}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn the_subscriber_role_follows_the_subscription() {
    let Some((env, shared)) = env_with_stripe().await else {
        return;
    };
    let member = env.member().verified().create().await;
    let (status, _) = subscribe(&env, &member).await;
    assert_eq!(status, StatusCode::OK);
    let customer = customer_of(&env, &member).await;
    let sub_id = format!("sub_{}", common::unique());

    shared.lock().unwrap().subscriptions.insert(
        sub_id.clone(),
        subscription(&sub_id, &customer, "active", "price_month"),
    );
    let (status, body) = webhook(
        &env,
        json!({"type": "checkout.session.completed",
               "data": {"object": {"id": "cs_unknown", "subscription": sub_id}}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = Call::get("/v1/me/subscription")
        .member(&member)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["subscriber"], true);
    assert_eq!(body["data"]["subscription"]["plan"], "monthly");
    assert_eq!(body["data"]["subscription"]["status"], "active");
    assert_eq!(body["data"]["can_manage"], true);

    // Already subscribed: no second checkout.
    let (status, body) = subscribe(&env, &member).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    // The same call twice changes nothing; a cancellation takes the role.
    shared.lock().unwrap().subscriptions.insert(
        sub_id.clone(),
        subscription(&sub_id, &customer, "canceled", "price_month"),
    );
    for _ in 0..2 {
        let (status, _) = webhook(
            &env,
            json!({"type": "customer.subscription.deleted",
                   "data": {"object": {"id": sub_id}}}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
    let (_, body) = Call::get("/v1/me/subscription")
        .member(&member)
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["subscriber"], false);
    assert_eq!(body["data"]["subscription"]["status"], "canceled");
}

#[tokio::test]
async fn an_unknown_subscription_is_retried_later() {
    let Some((env, _)) = env_with_stripe().await else {
        return;
    };
    // Stripe does not know it either: 503, so that Stripe calls again.
    let (status, _) = webhook(
        &env,
        json!({"type": "customer.subscription.updated",
               "data": {"object": {"id": "sub_missing"}}}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    // Events the API does not follow are acknowledged.
    let (status, _) = webhook(
        &env,
        json!({"type": "charge.refunded", "data": {"object": {}}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn the_portal_opens_for_a_customer_only() {
    let Some((env, _)) = env_with_stripe().await else {
        return;
    };
    let member = env.member().verified().create().await;
    let (status, body) = Call::post("/v1/billing/portal")
        .member(&member)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    subscribe(&env, &member).await;
    let customer = customer_of(&env, &member).await;
    let (status, body) = Call::post("/v1/billing/portal")
        .member(&member)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["data"]["url"],
        format!("https://billing.stripe.test/{customer}")
    );
}
