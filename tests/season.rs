//! The monthly duel season close (`docs/contract/brains.md` §3), against
//! PostgreSQL.
//!
//! Needs `TEST_DATABASE_URL` (skipped without it, unless
//! `REQUIRE_INTEGRATION=1`). Every test runs inside a transaction it rolls
//! back, on months long past: the shared database keeps no trace, and the
//! decay of real ratings is never committed.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use chrono::{DateTime, Utc};
use pulsars_api::game::rules::SubjectKey;
use pulsars_api::game::season::{self, Close, Month, Standing};
use pulsars_api::game::store;
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

fn database_url() -> Option<String> {
    match std::env::var("TEST_DATABASE_URL")
        .ok()
        .filter(|v| !v.is_empty())
    {
        Some(url) => Some(url),
        None if std::env::var("REQUIRE_INTEGRATION").as_deref() == Ok("1") => {
            panic!("REQUIRE_INTEGRATION=1 but TEST_DATABASE_URL is not set")
        }
        None => {
            eprintln!("skipped: TEST_DATABASE_URL is not set");
            None
        }
    }
}

async fn migrated(url: &str) -> PgPool {
    let pool = PgPool::connect(url).await.unwrap();
    store::migrate(&pool).await.expect("migrations apply");
    pool
}

fn at(rfc3339: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(rfc3339)
        .unwrap()
        .with_timezone(&Utc)
}

fn subject(key: &str) -> SubjectKey {
    SubjectKey::parse(key).unwrap()
}

/// `n` account ids no other test uses, ascending.
fn fresh_users(n: usize) -> Vec<i64> {
    let base = 800_000_000 + (Uuid::new_v4().as_u128() % 90_000_000) as i64;
    (0..n as i64).map(|i| base + i * 7).collect()
}

async fn rating(conn: &mut PgConnection, user_id: i64, subject: &str, rating: f64, rd: f64) {
    sqlx::query(
        "INSERT INTO game.ratings (user_id, subject, rating, rd, volatility, games, wins, losses, draws)
         VALUES ($1, $2, $3, $4, 0.06, 0, 0, 0, 0)",
    )
    .bind(user_id)
    .bind(subject)
    .bind(rating)
    .bind(rd)
    .execute(conn)
    .await
    .unwrap();
}

async fn rating_of(conn: &mut PgConnection, user_id: i64, subject: &str) -> (f64, f64) {
    sqlx::query_as("SELECT rating, rd FROM game.ratings WHERE user_id = $1 AND subject = $2")
        .bind(user_id)
        .bind(subject)
        .fetch_one(conn)
        .await
        .unwrap()
}

/// A finished match of `user_id` (one seat is enough for the standings).
async fn played(
    conn: &mut PgConnection,
    user_id: i64,
    subject: &str,
    ended_at: &str,
    ranked: bool,
    end_reason: &str,
) {
    let id = Uuid::now_v7();
    let ended = at(ended_at);
    sqlx::query(
        "INSERT INTO game.matches
                (id, mode, ranked, subject, rules, seed, created_at, started_at, ended_at, end_reason)
         VALUES ($1, 'duel', $2, $3, '{\"duration\": 60, \"target\": null, \"penalty\": false}',
                 1, $4, $4, $4, $5)",
    )
    .bind(id)
    .bind(ranked)
    .bind(subject)
    .bind(ended)
    .bind(end_reason)
    .execute(&mut *conn)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO game.match_players
                (match_id, seat, user_id, name, score, correct, answered, answer_time_ms, rank, outcome)
         VALUES ($1, 0, $2, 'p', 0, 0, 0, 0, 1, 'win')",
    )
    .bind(id)
    .bind(user_id)
    .execute(conn)
    .await
    .unwrap();
}

async fn ranked_in_march(conn: &mut PgConnection, user_id: i64, subject: &str, n: usize) {
    for day in 0..n {
        played(
            conn,
            user_id,
            subject,
            &format!("2001-03-{:02}T12:00:00Z", 10 + day),
            true,
            "completed",
        )
        .await;
    }
}

fn line(user_id: i64, rank: i64, rating: i64, ranked_matches: i64) -> Standing {
    Standing {
        user_id: user_id.to_string(),
        rank,
        rating,
        ranked_matches,
    }
}

#[tokio::test]
async fn standings_order_threshold_ties_and_utc_month() {
    let Some(url) = database_url() else { return };
    let pool = migrated(&url).await;
    let mut tx = pool.begin().await.unwrap();
    let cs = "computer-science";
    let u = fresh_users(8);

    // Rating order.
    rating(&mut tx, u[0], cs, 1700.4, 320.0).await;
    ranked_in_march(&mut tx, u[0], cs, 5).await;
    // Same rating: more ranked matches in the month first.
    rating(&mut tx, u[1], cs, 1600.0, 80.0).await;
    ranked_in_march(&mut tx, u[1], cs, 5).await;
    rating(&mut tx, u[2], cs, 1600.0, 80.0).await;
    ranked_in_march(&mut tx, u[2], cs, 7).await;
    // Same rating and matches: lower user_id first.
    rating(&mut tx, u[4], cs, 1550.0, 80.0).await;
    ranked_in_march(&mut tx, u[4], cs, 6).await;
    rating(&mut tx, u[3], cs, 1550.0, 80.0).await;
    ranked_in_march(&mut tx, u[3], cs, 6).await;

    // Best rating, but only 4 ranked matches in March in this subject.
    rating(&mut tx, u[5], cs, 1900.0, 60.0).await;
    rating(&mut tx, u[5], "maths", 1900.0, 60.0).await;
    ranked_in_march(&mut tx, u[5], cs, 4).await;
    played(
        &mut tx,
        u[5],
        cs,
        "2001-03-20T12:00:00Z",
        false,
        "completed",
    )
    .await;
    played(&mut tx, u[5], cs, "2001-03-21T12:00:00Z", true, "void").await;
    played(
        &mut tx,
        u[5],
        "maths",
        "2001-03-22T12:00:00Z",
        true,
        "completed",
    )
    .await;
    played(
        &mut tx,
        u[5],
        "all",
        "2001-03-22T12:00:00Z",
        true,
        "completed",
    )
    .await;
    played(&mut tx, u[5], cs, "2001-02-28T23:59:59Z", true, "completed").await;
    played(&mut tx, u[5], cs, "2001-04-01T00:00:00Z", true, "forfeit").await;

    // Exactly 5, two of them on the UTC month boundaries.
    rating(&mut tx, u[6], cs, 1400.0, 100.0).await;
    ranked_in_march(&mut tx, u[6], cs, 3).await;
    played(&mut tx, u[6], cs, "2001-03-01T00:00:00Z", true, "timeout").await;
    // 01:30 in UTC+2 on 1 April is still 31 March in UTC.
    played(
        &mut tx,
        u[6],
        cs,
        "2001-04-01T01:30:00+02:00",
        true,
        "completed",
    )
    .await;

    // Five matches but no rating row: cannot be ranked.
    ranked_in_march(&mut tx, u[7], cs, 5).await;

    let now = at("2001-04-01T00:00:04Z");
    let march = Month::new(2001, 3).unwrap();
    let close = season::close_month(&mut tx, march, subject(cs), now)
        .await
        .unwrap();
    let Close::Closed {
        standings,
        event_id,
    } = close
    else {
        panic!("not closed: {close:?}")
    };
    assert_eq!(
        standings,
        vec![
            line(u[0], 1, 1700, 5),
            line(u[2], 2, 1600, 7),
            line(u[1], 3, 1600, 5),
            line(u[3], 4, 1550, 6),
            line(u[4], 5, 1550, 6),
            line(u[6], 6, 1400, 5),
        ]
    );

    // The outbox row is the contract's event.
    let event_id = event_id.expect("an event for a non-empty standing");
    let (kind, status, body): (String, String, Value) =
        sqlx::query_as("SELECT type, status, body FROM game.webhook_outbox WHERE event_id = $1")
            .bind(event_id)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(
        (kind.as_str(), status.as_str()),
        ("season.ended", "pending")
    );
    assert_eq!(
        body,
        json!({
            "event_id": event_id.to_string(),
            "type": "season.ended",
            "version": 1,
            "occurred_at": "2001-04-01T00:00:04Z",
            "data": {
                "season": "2001-03",
                "subject": "computer-science",
                "standings": serde_json::to_value(&standings).unwrap(),
            },
        })
    );
    assert_eq!(body["data"]["standings"][0]["user_id"], u[0].to_string());
    assert_eq!(body["data"]["standings"][0]["rating"], 1700);

    // The season row.
    let (stored, stored_event, closed_at): (Value, Option<Uuid>, DateTime<Utc>) = sqlx::query_as(
        "SELECT standings, event_id, closed_at FROM game.seasons
              WHERE month = '2001-03' AND subject = 'computer-science'",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(stored, body["data"]["standings"]);
    assert_eq!(stored_event, Some(event_id));
    assert_eq!(closed_at, now);

    // Decay of every rating of the subject, ranked or not; other subjects
    // untouched.
    let decayed = rating_of(&mut tx, u[0], cs).await;
    assert!((decayed.0 - 1660.32).abs() < 1e-9, "{decayed:?}");
    assert_eq!(decayed.1, 350.0);
    assert_eq!(
        rating_of(&mut tx, u[5], cs).await,
        season::decay(1900.0, 60.0)
    );
    assert_eq!(rating_of(&mut tx, u[5], cs).await, (1820.0, 110.0));
    assert_eq!(rating_of(&mut tx, u[6], cs).await, (1420.0, 150.0));
    assert_eq!(rating_of(&mut tx, u[5], "maths").await, (1900.0, 60.0));

    // Closing again changes nothing: no second event, no second decay.
    let again = season::close_month(&mut tx, march, subject(cs), now)
        .await
        .unwrap();
    assert_eq!(again, Close::AlreadyClosed);
    let events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM game.webhook_outbox
          WHERE type = 'season.ended' AND body->'data'->>'season' = '2001-03'
            AND body->'data'->>'subject' = 'computer-science'",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(events, 1);
    assert_eq!(rating_of(&mut tx, u[6], cs).await, (1420.0, 150.0));

    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn an_empty_month_is_closed_without_an_event_and_still_decays() {
    let Some(url) = database_url() else { return };
    let pool = migrated(&url).await;
    let mut tx = pool.begin().await.unwrap();
    let u = fresh_users(1);
    rating(&mut tx, u[0], "physics", 1300.0, 330.0).await;

    let may = Month::new(2001, 5).unwrap();
    let close = season::close_month(&mut tx, may, subject("physics"), at("2001-06-01T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(
        close,
        Close::Closed {
            standings: vec![],
            event_id: None
        }
    );
    let events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM game.webhook_outbox
          WHERE type = 'season.ended' AND body->'data'->>'season' = '2001-05'",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(events, 0);
    let (standings, event_id): (Value, Option<Uuid>) = sqlx::query_as(
        "SELECT standings, event_id FROM game.seasons WHERE month = '2001-05' AND subject = 'physics'",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!((standings, event_id), (json!([]), None));
    assert_eq!(rating_of(&mut tx, u[0], "physics").await, (1340.0, 350.0));

    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn close_due_closes_the_previous_utc_month_of_every_subject_once() {
    let Some(url) = database_url() else { return };
    let pool = migrated(&url).await;
    let mut tx = pool.begin().await.unwrap();
    let u = fresh_users(1);
    rating(&mut tx, u[0], "all", 1700.0, 100.0).await;
    ranked_in_march(&mut tx, u[0], "all", 5).await;

    // Late on 31 March in UTC (already April in Paris): February closes.
    let done = season::close_due(&mut tx, at("2001-04-01T01:59:59+02:00"))
        .await
        .unwrap();
    assert_eq!(done.len(), 5);
    let feb: Vec<String> = sqlx::query_scalar(
        "SELECT subject FROM game.seasons WHERE month = '2001-02' ORDER BY subject",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    assert_eq!(
        feb,
        ["all", "chemistry", "computer-science", "maths", "physics"]
    );
    assert!(
        done.iter()
            .all(|(_, c)| matches!(c, Close::Closed { standings, .. } if standings.is_empty())),
        "{done:?}"
    );
    assert_eq!(rating_of(&mut tx, u[0], "all").await, (1660.0, 150.0));

    // At 00:00 UTC on 1 April, March closes, with its standing in `all`.
    let done = season::close_due(&mut tx, at("2001-04-01T00:00:00Z"))
        .await
        .unwrap();
    let all = done
        .iter()
        .find(|(s, _)| s.key() == "all")
        .map(|(_, c)| c.clone())
        .unwrap();
    let Close::Closed {
        standings,
        event_id: Some(_),
    } = all
    else {
        panic!("{all:?}")
    };
    // The rating before this close's decay, after February's.
    assert_eq!(standings, vec![line(u[0], 1, 1660, 5)]);
    assert_eq!(rating_of(&mut tx, u[0], "all").await, (1628.0, 200.0));

    // The ten-minute check that follows: nothing left to do.
    let again = season::close_due(&mut tx, at("2001-04-01T00:10:00Z"))
        .await
        .unwrap();
    assert!(
        again.iter().all(|(_, c)| *c == Close::AlreadyClosed),
        "{again:?}"
    );
    assert_eq!(rating_of(&mut tx, u[0], "all").await, (1628.0, 200.0));

    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn two_instances_close_a_month_once() {
    let Some(url) = database_url() else { return };
    let pool = migrated(&url).await;
    // Two connections race on a month nobody else uses, and commit: this
    // one decay of the chemistry ratings is real. Test databases only hold
    // leftovers of other tests there; the season row is deleted afterwards.
    let u = fresh_users(1);
    let month = Month::new(1999, 1 + (u[0] % 12) as u32).unwrap();
    let now = month.end();
    sqlx::query("DELETE FROM game.seasons WHERE month = $1")
        .bind(month.key())
        .execute(&pool)
        .await
        .unwrap();

    let (mut a, mut b) = (pool.acquire().await.unwrap(), pool.acquire().await.unwrap());
    let chem = subject("chemistry");
    let (ra, rb) = tokio::join!(
        season::close_month(&mut a, month, chem, now),
        season::close_month(&mut b, month, chem, now),
    );
    let results = [ra.unwrap(), rb.unwrap()];
    let closed = results
        .iter()
        .filter(|c| matches!(c, Close::Closed { .. }))
        .count();
    assert_eq!(closed, 1, "{results:?}");
    assert!(results.contains(&Close::AlreadyClosed));

    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM game.seasons WHERE month = $1 AND subject = 'chemistry'",
    )
    .bind(month.key())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rows, 1);

    sqlx::query("DELETE FROM game.seasons WHERE month = $1")
        .bind(month.key())
        .execute(&pool)
        .await
        .unwrap();
}
