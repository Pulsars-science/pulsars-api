//! Accounts linked to a provider (Google), in `site.comptes_sociaux`: the
//! rules of the site's `ConnexionSociale::resoudre` and
//! `ConnexionSocialeController::rattacher` (`docs/contract/auth-sessions.md`).

use sqlx::{PgPool, Postgres, Transaction};

/// The account linked to `subject` at `provider`.
pub async fn owner(
    pool: &PgPool,
    provider: &str,
    subject: &str,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT user_id FROM site.comptes_sociaux
          WHERE fournisseur = $1 AND identifiant_fournisseur = $2",
    )
    .bind(provider)
    .bind(subject)
    .fetch_optional(pool)
    .await
}

/// Links `user_id` to `subject` at `provider`, replacing the account's
/// previous identity there (Laravel's `updateOrCreate` on the pair
/// `(user_id, fournisseur)`).
pub async fn link(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i64,
    provider: &str,
    subject: &str,
    email: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO site.comptes_sociaux
                (user_id, fournisseur, identifiant_fournisseur, email, created_at, updated_at)
         VALUES ($1, $2, $3, $4, now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         ON CONFLICT (user_id, fournisseur) DO UPDATE
            SET identifiant_fournisseur = EXCLUDED.identifiant_fournisseur,
                email = EXCLUDED.email, updated_at = EXCLUDED.updated_at",
    )
    .bind(user_id)
    .bind(provider)
    .bind(subject)
    .bind(email)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// The providers linked to `user_id`.
pub async fn providers(pool: &PgPool, user_id: i64) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT fournisseur::text FROM site.comptes_sociaux WHERE user_id = $1 ORDER BY fournisseur",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn unlink(pool: &PgPool, user_id: i64, provider: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM site.comptes_sociaux WHERE user_id = $1 AND fournisseur = $2")
        .bind(user_id)
        .bind(provider)
        .execute(pool)
        .await?;
    Ok(())
}
