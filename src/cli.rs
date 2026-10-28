//! One-off commands for a new server (`docs/deployment.md`):
//!
//! - `pulsars-api create-admin <pseudo> <email>`: the first account, with
//!   the `super-admin` role, its address verified and the welcome steps
//!   done. The password is read from standard input, never from the command
//!   line.
//! - `pulsars-api import-content <url of the old database>`: copies the
//!   content of the old site (courses, exams, resources, news, files and
//!   their translations) into this database, every author becoming the
//!   administrator. Accounts, the forum and the social part are not copied.

use sqlx::{PgPool, Postgres, Row as _, Transaction};

use crate::auth::password;

/// The content tables, parents first. Their columns are the same here and in
/// the old database, where they sit in `site` (its older layout) or
/// `contenu`.
pub const CONTENT_TABLES: &[&str] = &[
    "courses",
    "chapters",
    "lessons",
    "exercises",
    "exams",
    "exam_parties",
    "ressources",
    "actualites",
    "fichiers",
    "contenu_translations",
    "telechargements",
];

/// The account `create-admin` makes: verified, welcomed, super admin.
pub async fn create_admin(
    pool: &PgPool,
    memory_kib: u32,
    pseudo: &str,
    email: &str,
    plain: String,
) -> Result<i64, String> {
    if plain.chars().count() < 12 {
        return Err("the password needs at least 12 characters".to_owned());
    }
    let hash = password::hash(memory_kib, plain)
        .await
        .map_err(|e| e.to_string())?;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let taken: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT FROM site.users WHERE lower(pseudo) = lower($1) OR lower(email) = lower($2))",
    )
    .bind(pseudo)
    .bind(email)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    if taken {
        return Err(format!("an account already uses `{pseudo}` or `{email}`"));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO site.users (pseudo, name, email, password, email_verified_at, onboarding_at,
                                 locale, created_at, updated_at)
         VALUES ($1, $1, $2, $3, now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC', 'fr',
                 now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         RETURNING id",
    )
    .bind(pseudo)
    .bind(email)
    .bind(&hash)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    let granted = sqlx::query(
        "INSERT INTO site.role_user (role_id, user_id)
         SELECT id, $1 FROM site.roles WHERE slug = 'super-admin'",
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    if granted.rows_affected() != 1 {
        return Err("no `super-admin` role: run `pulsars-api migrate` first".to_owned());
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(id)
}

/// What `import-content` copied: rows per table.
#[derive(Debug, Default)]
pub struct Imported {
    pub tables: Vec<(&'static str, u64)>,
}

/// The schema of `table` in the old database: `contenu` once the content
/// moved there, `site` before.
async fn source_schema(source: &PgPool, table: &str) -> Result<Option<String>, String> {
    sqlx::query_scalar(
        "SELECT table_schema::text FROM information_schema.tables
          WHERE table_name = $1 AND table_schema IN ('contenu', 'site')
          ORDER BY table_schema = 'contenu' DESC LIMIT 1",
    )
    .bind(table)
    .fetch_optional(source)
    .await
    .map_err(|e| e.to_string())
}

async fn columns(pool: &PgPool, schema: &str, table: &str) -> Result<Vec<String>, String> {
    sqlx::query_scalar(
        "SELECT column_name::text FROM information_schema.columns
          WHERE table_schema = $1 AND table_name = $2 ORDER BY ordinal_position",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())
}

/// Columns of `contenu.<table>` that point at `site.users`.
async fn user_columns(dest: &PgPool, table: &str) -> Result<Vec<String>, String> {
    sqlx::query_scalar(
        "SELECT a.attname::text
           FROM pg_constraint c
           JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = ANY (c.conkey)
          WHERE c.contype = 'f' AND c.conrelid = format('contenu.%I', $1::text)::regclass
            AND c.confrelid = 'site.users'::regclass",
    )
    .bind(table)
    .fetch_all(dest)
    .await
    .map_err(|e| e.to_string())
}

fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Rows read and written per batch: files are megabytes each, so they go
/// a few at a time.
fn batch_size(table: &str) -> i64 {
    if table == "fichiers" { 4 } else { 500 }
}

/// Copies the content of `source` into `dest`, in one transaction, all
/// authors becoming `admin_id`. Refuses when `dest` already has content.
pub async fn import_content(
    source: &PgPool,
    dest: &PgPool,
    admin_id: i64,
) -> Result<Imported, String> {
    let existing: i64 = sqlx::query_scalar("SELECT count(*) FROM contenu.courses")
        .fetch_one(dest)
        .await
        .map_err(|e| e.to_string())?;
    if existing > 0 {
        return Err("this database already has courses: import into an empty one".to_owned());
    }
    let mut tx: Transaction<'_, Postgres> = dest.begin().await.map_err(|e| e.to_string())?;
    let mut imported = Imported::default();
    for &table in CONTENT_TABLES {
        let Some(schema) = source_schema(source, table).await? else {
            imported.tables.push((table, 0));
            continue;
        };
        let from = columns(source, &schema, table).await?;
        let to = columns(dest, "contenu", table).await?;
        let shared: Vec<&String> = to.iter().filter(|c| from.contains(c)).collect();
        let users = user_columns(dest, table).await?;
        // Downloads keep their count, not who downloaded.
        let select: Vec<String> = shared
            .iter()
            .map(|c| {
                if users.contains(c) && table == "telechargements" {
                    "NULL".to_owned()
                } else if users.contains(c) {
                    format!(
                        "CASE WHEN r.{q} IS NULL THEN NULL ELSE $2 END",
                        q = quoted(c)
                    )
                } else {
                    format!("r.{}", quoted(c))
                }
            })
            .collect();
        let list = shared
            .iter()
            .map(|c| quoted(c))
            .collect::<Vec<_>>()
            .join(", ");
        let insert = format!(
            "INSERT INTO contenu.{t} ({list})
             SELECT {sel} FROM jsonb_populate_recordset(NULL::contenu.{t}, $1) r",
            t = quoted(table),
            sel = select.join(", ")
        );
        let read = format!(
            "SELECT COALESCE(json_agg(row_to_json(s)), '[]'::json)
               FROM (SELECT * FROM {s}.{t} ORDER BY id LIMIT $1 OFFSET $2) s",
            s = quoted(&schema),
            t = quoted(table)
        );
        let size = batch_size(table);
        let mut offset = 0i64;
        let mut copied = 0u64;
        loop {
            let rows: serde_json::Value = sqlx::query_scalar(sqlx::AssertSqlSafe(read.clone()))
                .bind(size)
                .bind(offset)
                .fetch_one(source)
                .await
                .map_err(|e| format!("reading {schema}.{table}: {e}"))?;
            let count = rows.as_array().map_or(0, Vec::len);
            if count == 0 {
                break;
            }
            let done = sqlx::query(sqlx::AssertSqlSafe(insert.clone()))
                .bind(&rows)
                .bind(admin_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("writing contenu.{table}: {e}"))?;
            copied += done.rows_affected();
            offset += size;
        }
        // The next id after the copied ones.
        let reset = format!(
            "SELECT setval(pg_get_serial_sequence('contenu.{t}', 'id'),
                           COALESCE((SELECT max(id) FROM contenu.{t}), 0) + 1, false)",
            t = quoted(table)
        );
        sqlx::query(sqlx::AssertSqlSafe(reset))
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("sequence of contenu.{table}: {e}"))?;
        imported.tables.push((table, copied));
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(imported)
}

/// The id of the account `pseudo`, for `import-content --author`.
pub async fn account_id(pool: &PgPool, pseudo: &str) -> Result<Option<i64>, String> {
    sqlx::query("SELECT id FROM site.users WHERE lower(pseudo) = lower($1)")
        .bind(pseudo)
        .fetch_optional(pool)
        .await
        .map(|row| row.map(|r| r.get::<i64, _>(0)))
        .map_err(|e| e.to_string())
}
