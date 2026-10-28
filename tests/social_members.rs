//! The member directory, public profiles, follows and friends
//! (`src/routes/social/{members,friends}.rs`). Ported from the site's
//! `AmitieMessagerieTest`, `AbonnementTest`, `AmiQrTest`, `CerveauxTest`
//! and `ExperienceTest`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{Call, Env, TestMember, codes, unique};
use serde_json::{Value, json};

async fn friends_between(env: &Env, a: &TestMember, b: &TestMember) -> Vec<(bool, String)> {
    let (one, two) = if a.id < b.id {
        (a.id, b.id)
    } else {
        (b.id, a.id)
    };
    sqlx::query_as(
        "SELECT accepted_at IS NOT NULL, origine FROM site.friendships
          WHERE user_one_id = $1 AND user_two_id = $2",
    )
    .bind(one)
    .bind(two)
    .fetch_all(&env.owner)
    .await
    .unwrap()
}

/// A friendship planted as `requester` asking `other`, accepted or not.
async fn plant_friendship(
    env: &Env,
    requester: &TestMember,
    other: &TestMember,
    accepted: bool,
) -> i64 {
    let (one, two) = if requester.id < other.id {
        (requester.id, other.id)
    } else {
        (other.id, requester.id)
    };
    sqlx::query_scalar(
        "INSERT INTO site.friendships (user_one_id, user_two_id, requested_by_id, accepted_at,
                                       created_at, updated_at)
         VALUES ($1, $2, $3, CASE WHEN $4 THEN now() AT TIME ZONE 'UTC' END,
                 now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         RETURNING id",
    )
    .bind(one)
    .bind(two)
    .bind(requester.id)
    .bind(accepted)
    .fetch_one(&env.owner)
    .await
    .unwrap()
}

async fn follows(env: &Env, a: &TestMember, b: &TestMember) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM site.abonnements WHERE abonne_id = $1 AND suivi_id = $2)",
    )
    .bind(a.id)
    .bind(b.id)
    .fetch_one(&env.owner)
    .await
    .unwrap()
}

// --- Profiles --------------------------------------------------------------

#[tokio::test]
async fn a_public_profile_never_shows_the_email() {
    let Some(env) = Env::new().await else { return };
    let camille = env.member().verified().role("moderateur").create().await;
    sqlx::query("UPDATE site.users SET bio = 'Je révise les mathématiques.', mute_jusqu_a = now() + interval '1 day', mute_motif = 'secret' WHERE id = $1")
        .bind(camille.id)
        .execute(&env.owner)
        .await
        .unwrap();

    let (status, headers, body) = Call::get(format!("/v1/members/{}", camille.pseudo))
        .send_full(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(headers["cache-control"], "no-store");
    let profile = &body["data"];
    assert_eq!(profile["pseudo"], camille.pseudo.as_str());
    assert_eq!(profile["id"], camille.id.to_string());
    assert_eq!(profile["bio"], "Je révise les mathématiques.");
    assert_eq!(profile["roles"][0]["slug"], "moderateur");
    assert_eq!(profile["roles"][0]["level"], 1);
    assert!(profile["member_since"].is_string());
    assert_eq!(
        body["data"]["friendship"],
        json!({"state": "visitor", "id": null})
    );
    assert_eq!(body["data"]["following"], Value::Null);
    let text = body.to_string();
    for private in [camille.email.as_str(), "secret", "mute", "locale", "\"fr\""] {
        assert!(!text.contains(private), "{private} in {text}");
    }

    // Someone else signed in: the same, with their standing.
    let other = env.member().verified().create().await;
    let (_, body) = Call::get(format!("/v1/members/{}", camille.pseudo))
        .member(&other)
        .send(&env.app)
        .await;
    assert_eq!(
        body["data"]["friendship"],
        json!({"state": "none", "id": null})
    );
    assert_eq!(body["data"]["following"], json!({"following": false}));
    assert!(!body.to_string().contains(&camille.email));

    let (status, _) = Call::get(format!("/v1/members/{}", unique()))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_profile_shows_the_experience_and_the_forum_activity() {
    let Some(env) = Env::new().await else { return };
    let alice = env.member().verified().create().await;
    let (_, body) = Call::get(format!("/v1/members/{}", alice.pseudo))
        .send(&env.app)
        .await;
    let xp = &body["data"]["xp"];
    assert_eq!(
        (xp["total"].as_i64(), xp["level"].as_i64()),
        (Some(0), Some(1))
    );
    assert_eq!(xp["breakdown"].as_array().unwrap().len(), 7);

    // Two questions, one of them deleted: both count for XP, only the live
    // one on the profile. Eight games on one day, two won (34 XP): capped
    // at 30.
    for deleted in [false, true] {
        sqlx::query(
            "INSERT INTO site.questions (forum_theme_id, user_id, titre, slug, corps, created_at, deleted_at)
             SELECT id, $1, 't', $2, 'c', now(), CASE WHEN $3 THEN now() END
               FROM site.forum_themes ORDER BY id LIMIT 1",
        )
        .bind(alice.id)
        .bind(unique())
        .bind(deleted)
        .execute(&env.owner)
        .await
        .unwrap();
    }
    for i in 0..8 {
        sqlx::query(
            "INSERT INTO site.game_results (match_id, user_id, mode, ranked, outcome, ended_at)
             VALUES ($1, $2, 'duel', true, $3, '2026-10-20 12:00:00')",
        )
        .bind(uuid::Uuid::new_v4())
        .bind(alice.id)
        .bind(if i < 2 { "win" } else { "loss" })
        .execute(&env.owner)
        .await
        .unwrap();
    }
    let (_, body) = Call::get(format!("/v1/members/{}", alice.pseudo))
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["question_count"], 1);
    assert_eq!(body["data"]["answer_count"], 0);
    let xp = &body["data"]["xp"];
    assert_eq!(xp["total"], 2 * 4 + 30, "{xp}");
    let games = &xp["breakdown"][6];
    assert_eq!(
        games,
        &json!({"key": "games", "count": 8, "unit_points": 3, "xp": 30})
    );
    // The same total as the view every other route reads.
    let view: i32 = sqlx::query_scalar("SELECT total FROM site.member_xp WHERE user_id = $1")
        .bind(alice.id)
        .fetch_one(&env.db)
        .await
        .unwrap();
    assert_eq!(xp["total"], view);
}

#[tokio::test]
async fn brains_show_as_the_member_chose() {
    let Some(env) = Env::new().await else { return };
    let member = env.member().verified().create().await;
    let visitor = env.member().verified().create().await;
    sqlx::query(
        "INSERT INTO site.cerveaux_mouvements (user_id, montant, raison, cle, created_at)
         VALUES ($1, 120, 'saison', $2, now())",
    )
    .bind(member.id)
    .bind(format!("saison:{}", unique()))
    .execute(&env.owner)
    .await
    .unwrap();
    let url = format!("/v1/members/{}", member.pseudo);

    let (_, body) = Call::get(&url).member(&visitor).send(&env.app).await;
    assert_eq!(
        body["data"]["brains"],
        json!({"balance": 120, "private": false})
    );
    let (_, body) = Call::get(&url).send(&env.app).await;
    assert_eq!(body["data"]["brains"]["balance"], 120);

    sqlx::query("UPDATE site.users SET cerveaux_publics = false WHERE id = $1")
        .bind(member.id)
        .execute(&env.owner)
        .await
        .unwrap();
    let (_, body) = Call::get(&url).send(&env.app).await;
    assert_eq!(body["data"]["brains"], Value::Null);
    let (_, body) = Call::get(&url).member(&visitor).send(&env.app).await;
    assert_eq!(body["data"]["brains"], Value::Null);
    let (_, body) = Call::get(&url).member(&member).send(&env.app).await;
    assert_eq!(
        body["data"]["brains"],
        json!({"balance": 120, "private": true})
    );
    assert_eq!(
        body["data"]["friendship"],
        json!({"state": "self", "id": null})
    );
    assert_eq!(body["data"]["following"], Value::Null);
}

#[tokio::test]
async fn the_search_lists_nobody_until_one_searches() {
    let Some(env) = Env::new().await else { return };
    let tag = unique();
    let camille = env.member().verified().create().await;
    let benoit = env.member().verified().create().await;
    for (member, pseudo) in [
        (&camille, format!("Camille{tag}")),
        (&benoit, format!("benoit{tag}")),
    ] {
        sqlx::query("UPDATE site.users SET pseudo = $2 WHERE id = $1")
            .bind(member.id)
            .bind(pseudo)
            .execute(&env.owner)
            .await
            .unwrap();
    }

    let (status, body) = Call::get("/v1/members").send(&env.app).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({"data": [], "next_cursor": null, "total": 0}));
    let (_, body) = Call::get("/v1/members?q=%20").send(&env.app).await;
    assert_eq!(body["total"], 0);

    // A piece of the pseudo, ignoring case.
    let (status, body) = Call::get(format!("/v1/members?q=CAMILLE{}", tag.to_uppercase()))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], 1, "{body}");
    let (_, body) = Call::get(format!("/v1/members?q=ille{}", &tag[..4]))
        .send(&env.app)
        .await;
    assert_eq!(body["total"], 1, "{body}");
    assert_eq!(body["data"][0]["pseudo"], format!("Camille{tag}"));
    assert!(body["data"][0].get("email").is_none());
    let (_, body) = Call::get(format!("/v1/members?q={tag}"))
        .send(&env.app)
        .await;
    // In the database's collation order, as Laravel's `orderBy`.
    assert_eq!(body["total"], 2);
    assert_eq!(body["data"].as_array().unwrap().len(), 2);
    // `%` and `_` are literal.
    let (_, body) = Call::get("/v1/members?q=%25").send(&env.app).await;
    assert_eq!(body["total"], 0);
    let (status, _) = Call::get("/v1/members?page=0&q=a").send(&env.app).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// --- Follows -----------------------------------------------------------------

#[tokio::test]
async fn following_is_one_way_and_a_mutual_follow_makes_friends() {
    let Some(env) = Env::new().await else { return };
    let alice = env.member().verified().create().await;
    let bob = env.member().verified().create().await;

    let (status, body) = Call::post(format!("/v1/members/{}/follow", bob.pseudo))
        .member(&alice)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["data"],
        json!({"following": true, "friendship": {"state": "none", "id": null}})
    );
    assert!(follows(&env, &alice, &bob).await);
    assert!(!follows(&env, &bob, &alice).await);
    assert!(friends_between(&env, &alice, &bob).await.is_empty());

    // Bob follows back (PUT works as POST): friends, born of the follow.
    let (_, body) = Call::put(format!("/v1/members/{}/follow", alice.pseudo))
        .member(&bob)
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["friendship"]["state"], "friends");
    assert_eq!(
        friends_between(&env, &alice, &bob).await,
        [(true, "abonnement".to_owned())]
    );
    let (_, body) = Call::get(format!("/v1/members/{}", bob.pseudo))
        .member(&alice)
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["following"], json!({"following": true}));

    // Alice stops: the friendship goes, Bob still follows her.
    let (status, body) = Call::delete(format!("/v1/members/{}/follow", bob.pseudo))
        .member(&alice)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["data"],
        json!({"following": false, "friendship": {"state": "none", "id": null}})
    );
    assert!(friends_between(&env, &alice, &bob).await.is_empty());
    assert!(follows(&env, &bob, &alice).await);
}

#[tokio::test]
async fn unfollowing_keeps_a_friendship_born_of_a_request() {
    let Some(env) = Env::new().await else { return };
    let alice = env.member().verified().create().await;
    let bob = env.member().verified().create().await;
    plant_friendship(&env, &alice, &bob, true).await;
    for (who, whom) in [(&alice, &bob), (&bob, &alice)] {
        Call::post(format!("/v1/members/{}/follow", whom.pseudo))
            .member(who)
            .send(&env.app)
            .await;
    }
    Call::delete(format!("/v1/members/{}/follow", bob.pseudo))
        .member(&alice)
        .send(&env.app)
        .await;
    assert_eq!(
        friends_between(&env, &alice, &bob).await,
        [(true, "demande".to_owned())]
    );
}

#[tokio::test]
async fn a_mutual_follow_accepts_a_pending_request() {
    let Some(env) = Env::new().await else { return };
    let alice = env.member().verified().create().await;
    let bob = env.member().verified().create().await;
    plant_friendship(&env, &alice, &bob, false).await;
    for (who, whom) in [(&alice, &bob), (&bob, &alice)] {
        Call::post(format!("/v1/members/{}/follow", whom.pseudo))
            .member(who)
            .send(&env.app)
            .await;
    }
    assert_eq!(
        friends_between(&env, &alice, &bob).await,
        [(true, "demande".to_owned())]
    );
}

#[tokio::test]
async fn following_needs_a_verified_member_and_never_oneself() {
    let Some(env) = Env::new().await else { return };
    let alice = env.member().verified().create().await;
    let fresh = env.member().create().await;
    let url = format!("/v1/members/{}/follow", alice.pseudo);

    let (status, body) = Call::post(&url).member(&alice).send(&env.app).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["following"], false);
    assert!(!follows(&env, &alice, &alice).await);

    let (status, _) = Call::post(&url).send(&env.app).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = Call::post(&url).member(&fresh).send(&env.app).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = Call::post(format!("/v1/members/{}/follow", unique()))
        .member(&alice)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// --- Friends -----------------------------------------------------------------

#[tokio::test]
async fn a_friend_request_is_unique_and_accepted_by_its_recipient() {
    let Some(env) = Env::new().await else { return };
    let alice = env.member().verified().create().await;
    let bob = env.member().verified().create().await;

    let (status, body) = Call::post(format!("/v1/members/{}/friend-requests", bob.pseudo))
        .member(&alice)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["data"]["state"], "sent");
    let id = body["data"]["id"].as_i64().unwrap();

    // A request the other way never makes a second pair.
    let (status, body) = Call::post(format!("/v1/members/{}/friend-requests", alice.pseudo))
        .member(&bob)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["reason"], "request_pending");
    assert_eq!(friends_between(&env, &alice, &bob).await.len(), 1);

    let (_, body) = Call::get(format!("/v1/members/{}", alice.pseudo))
        .member(&bob)
        .send(&env.app)
        .await;
    assert_eq!(
        body["data"]["friendship"],
        json!({"state": "received", "id": id})
    );

    // Not one's own request.
    let (status, _) = Call::post(format!("/v1/friends/{id}/accept"))
        .member(&alice)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = Call::post(format!("/v1/friends/{id}/accept"))
        .member(&bob)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"], json!({"state": "friends", "id": id}));
    assert_eq!(
        friends_between(&env, &alice, &bob).await,
        [(true, "demande".to_owned())]
    );

    let (status, body) = Call::post(format!("/v1/members/{}/friend-requests", bob.pseudo))
        .member(&alice)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["state"], "friends");
}

#[tokio::test]
async fn one_cannot_befriend_oneself_nor_touch_others_friendships() {
    let Some(env) = Env::new().await else { return };
    let alice = env.member().verified().create().await;
    let bob = env.member().verified().create().await;
    let eve = env.member().verified().create().await;

    let (status, body) = Call::post(format!("/v1/members/{}/friend-requests", alice.pseudo))
        .member(&alice)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(codes(&body), [("pseudo".to_owned(), "self".to_owned())]);

    let id = plant_friendship(&env, &alice, &bob, false).await;
    for call in [
        Call::post(format!("/v1/friends/{id}/accept")),
        Call::delete(format!("/v1/friends/{id}")),
    ] {
        let (status, _) = call.member(&eve).send(&env.app).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    let (status, _) = Call::delete("/v1/friends/abc")
        .member(&eve)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(friends_between(&env, &alice, &bob).await.len(), 1);

    // A sent request is withdrawn by its author.
    let (status, _) = Call::delete(format!("/v1/friends/{id}"))
        .member(&alice)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(friends_between(&env, &alice, &bob).await.is_empty());
}

#[tokio::test]
async fn the_friends_page_sorts_friends_and_requests() {
    let Some(env) = Env::new().await else { return };
    let me = env.member().verified().create().await;
    let friend = env.member().verified().create().await;
    let asking = env.member().verified().create().await;
    let asked = env.member().verified().create().await;
    let friendship = plant_friendship(&env, &friend, &me, true).await;
    plant_friendship(&env, &asking, &me, false).await;
    plant_friendship(&env, &me, &asked, false).await;

    let (status, body) = Call::get("/v1/friends").member(&me).send(&env.app).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pseudos = |list: &str| -> Vec<String> {
        body["data"][list]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["member"]["pseudo"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(pseudos("friends"), [friend.pseudo.as_str()]);
    assert_eq!(pseudos("received"), [asking.pseudo.as_str()]);
    assert_eq!(pseudos("sent"), [asked.pseudo.as_str()]);
    assert_eq!(body["data"]["friends"][0]["friendship_id"], friendship);
    assert!(body["data"]["friends"][0]["since"].is_string());

    let (status, _) = Call::get("/v1/friends").send(&env.app).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn friend_requests_are_rate_limited() {
    let Some(env) = Env::new().await else { return };
    let alice = env.member().verified().create().await;
    let bob = env.member().verified().create().await;
    let mut last = StatusCode::OK;
    for _ in 0..21 {
        (last, _) = Call::post(format!("/v1/members/{}/friend-requests", bob.pseudo))
            .member(&alice)
            .send(&env.app)
            .await;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
}

// --- QR code -----------------------------------------------------------------

async fn qr(env: &Env, member: &TestMember) -> String {
    let (status, headers, body) = Call::get("/v1/me/friend-qr")
        .member(member)
        .send_full(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(headers["cache-control"], "no-store");
    let token = body["data"]["token"].as_str().unwrap().to_owned();
    assert_eq!(
        body["data"]["url"],
        format!("https://pulsars.science/ami/{token}")
    );
    token
}

#[tokio::test]
async fn the_qr_token_is_stable_until_renewed() {
    let Some(env) = Env::new().await else { return };
    let me = env.member().verified().create().await;
    sqlx::query("UPDATE site.users SET friend_token = NULL WHERE id = $1")
        .bind(me.id)
        .execute(&env.owner)
        .await
        .unwrap();
    let token = qr(&env, &me).await;
    assert_eq!(token.len(), 24);
    assert!(
        token
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    );
    assert_eq!(qr(&env, &me).await, token);

    let (status, body) = Call::post("/v1/me/friend-qr/regenerate")
        .member(&me)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    let renewed = body["data"]["token"].as_str().unwrap().to_owned();
    assert_ne!(renewed, token);
    assert_eq!(qr(&env, &me).await, renewed);

    // The old link leads nowhere.
    let other = env.member().verified().create().await;
    let (status, _) = Call::post("/v1/friends/scan")
        .member(&other)
        .json(json!({"token": token}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = Call::get("/v1/me/friend-qr").send(&env.app).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn scanning_makes_friends_at_once_and_only_once() {
    let Some(env) = Env::new().await else { return };
    let owner = env.member().verified().create().await;
    let me = env.member().verified().create().await;
    let token = qr(&env, &owner).await;
    // A request was waiting: the scan accepts it.
    plant_friendship(&env, &me, &owner, false).await;

    let scan = || {
        Call::post("/v1/friends/scan")
            .member(&me)
            .json(json!({"token": token}))
    };
    let (status, body) = scan().send(&env.app).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["state"], "friends");
    assert_eq!(body["data"]["member"]["pseudo"], owner.pseudo.as_str());
    let (_, body) = scan().send(&env.app).await;
    assert_eq!(body["data"]["state"], "already_friends");
    assert_eq!(
        friends_between(&env, &me, &owner).await,
        [(true, "demande".to_owned())]
    );

    let stranger = env.member().verified().create().await;
    let (status, body) = Call::post("/v1/friends/scan")
        .member(&stranger)
        .json(json!({"token": token}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["state"], "friends");
    assert_eq!(
        friends_between(&env, &stranger, &owner).await,
        [(true, "demande".to_owned())]
    );
}

#[tokio::test]
async fn scanning_ones_own_or_an_unknown_code_does_nothing() {
    let Some(env) = Env::new().await else { return };
    let me = env.member().verified().create().await;
    let token = qr(&env, &me).await;
    let (status, body) = Call::post("/v1/friends/scan")
        .member(&me)
        .json(json!({"token": token}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(codes(&body), [("token".to_owned(), "self".to_owned())]);
    for token in ["jeton-qui-nexiste-pas", "", &"x".repeat(200)] {
        let (status, _) = Call::post("/v1/friends/scan")
            .member(&me)
            .json(json!({"token": token}))
            .send(&env.app)
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{token}");
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM site.friendships WHERE user_one_id = $1 OR user_two_id = $1",
    )
    .bind(me.id)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    assert_eq!(count, 0);
    let (status, _) = Call::post("/v1/friends/scan")
        .json(json!({"token": "x"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
