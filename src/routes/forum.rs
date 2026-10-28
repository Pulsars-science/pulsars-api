//! `/v1/forum/*` (`openapi.yaml`, tag `forum`): the public questions and
//! answers forum, as Laravel's `ForumController`, `QuestionController`,
//! `AnswerController`, `VoteForumController`, `SignalementForumController`
//! and `TraductionForumController` served it.
//!
//! - Reading is open to everyone (`MaybeMember`): the reader's own votes
//!   and, for a moderator, the body of hidden messages change with them.
//! - Writing needs a verified member who finished the welcome steps
//!   (`Verified`); asking and answering are refused while muted.
//! - Bodies are the Markdown the member wrote (`body_md`, with LaTeX); the
//!   site renders it, as for lessons. A hidden message keeps its title but
//!   not its body, except for moderators.
//! - The XP of questions, answers and accepted answers is counted by the
//!   `site.member_xp` view, not here.

use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::http::StatusCode;
use axum::response::Response;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::auth::{Caller, Data, limit, no_store};
use super::courses::timestamp;
use super::params::Params;
use super::public::Page;
use crate::auth::member::{MaybeMember, Member, Verified};
use crate::db::forum::{self, AnswerRow, QuestionRow, Sort, Target, ThemeRow, TranslationRow};
use crate::deepl;
use crate::error::{ApiError, ParameterError};
use crate::extract::Json;
use crate::rate_limit;
use crate::state::AppState;

/// Questions per page, as Laravel's `paginate(20)`.
const PER_PAGE: u32 = 20;
/// Characters of a question's excerpt in lists (`Str::limit(…, 160)`).
const EXCERPT_CHARS: usize = 160;
/// Characters of the body read for an excerpt: room for the tags
/// `strip_tags` removes.
const EXCERPT_SOURCE_CHARS: u32 = 1280;
/// A view counts once per reader and question in this window, Laravel's
/// session lifetime (`SESSION_LIFETIME`, 120 minutes).
const VIEW_WINDOW_SECS: i64 = 120 * 60;
/// Reports per minute per member (`throttle:20,1`).
const REPORT_LIMIT: u32 = 20;
/// Translations per minute per reader (`throttle:10,1`).
const TRANSLATE_LIMIT: u32 = 10;
/// The languages a post can be translated into (`config/localisation.php`).
const TRANSLATION_LANGS: &[&str] = &["en", "fr", "ru"];
/// Why a forum message is reported (`TypeSignalement::pourForum`).
pub const REPORT_TYPES: &[&str] = &["spam", "abus", "hors_sujet"];
const SORTS: &[&str] = &["recent", "popular", "unanswered"];

// --- Shapes -----------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct Theme {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub color: String,
    pub question_count: i64,
}

impl From<ThemeRow> for Theme {
    fn from(row: ThemeRow) -> Self {
        Self {
            slug: row.slug,
            name: row.name,
            description: row.description,
            color: row.color,
            question_count: row.question_count,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ThemeRef {
    pub slug: String,
    pub name: String,
    pub color: String,
}

/// Laravel's `auteur` and `auteur_id`: the pseudo, `null` once the
/// account is deleted (the site shows « Compte supprimé »), and its id.
fn author(user_id: Option<i64>, pseudo: Option<String>) -> (Option<String>, Option<String>) {
    match user_id {
        Some(id) => (pseudo, Some(id.to_string())),
        None => (None, None),
    }
}

#[derive(Debug, Serialize)]
pub struct QuestionSummary {
    pub id: i64,
    pub slug: String,
    pub title: String,
    /// Plain text, at most 160 characters; `null` for a hidden question,
    /// except for moderators.
    pub excerpt: Option<String>,
    pub theme: ThemeRef,
    pub author: Option<String>,
    pub author_id: Option<String>,
    pub votes: i64,
    pub answer_count: i64,
    pub views: i32,
    pub solved: bool,
    pub has_voted: bool,
    pub is_hidden: bool,
    pub is_locked: bool,
    pub is_pinned: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// A question with its answers, as `Forum/Question.vue` shows it.
#[derive(Debug, Serialize)]
pub struct QuestionDetail {
    pub id: i64,
    pub slug: String,
    pub title: String,
    /// `null` for a hidden question, except for moderators.
    pub body_md: Option<String>,
    pub theme: ThemeRef,
    pub author: Option<String>,
    pub author_id: Option<String>,
    pub votes: i64,
    pub views: i32,
    pub has_voted: bool,
    pub accepted_answer_id: Option<i64>,
    pub is_hidden: bool,
    pub hidden_reason: Option<String>,
    pub is_locked: bool,
    pub is_pinned: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// Every answer of the question, replies included.
    pub answer_count: usize,
    /// Answers to the question, oldest first, each with its replies.
    pub answers: Vec<AnswerNode>,
    pub viewer: Viewer,
}

#[derive(Debug, Serialize)]
pub struct Viewer {
    pub is_author: bool,
    /// A member who is not muted, on a question that is not locked.
    pub can_participate: bool,
    pub is_moderator: bool,
}

#[derive(Debug, Serialize)]
pub struct AnswerNode {
    pub id: i64,
    /// `null` for a hidden answer, except for moderators.
    pub body_md: Option<String>,
    pub author: Option<String>,
    pub author_id: Option<String>,
    pub votes: i64,
    pub has_voted: bool,
    pub is_hidden: bool,
    pub hidden_reason: Option<String>,
    pub created_at: Option<String>,
    /// Replies to this answer, oldest first.
    pub children: Vec<AnswerNode>,
}

#[derive(Debug, Serialize)]
struct Created {
    id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    slug: Option<String>,
}

#[derive(Debug, Serialize)]
struct Accepted {
    accepted_answer_id: Option<i64>,
}

#[derive(Debug, Serialize)]
struct Vote {
    voted: bool,
    votes: i64,
}

#[derive(Debug, Serialize)]
pub struct Translation {
    /// `null` for an answer.
    pub title: Option<String>,
    pub body_md: String,
    pub source_lang: Option<String>,
    pub target_lang: String,
    /// Whether it came from `site.forum_translations` rather than DeepL.
    pub cached: bool,
}

// --- Bodies -----------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AskBody {
    theme: String,
    title: String,
    body: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AnswerBody {
    body: String,
    parent_id: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReportBody {
    target_type: String,
    target_id: Option<i64>,
    #[serde(rename = "type")]
    kind: String,
    message: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TranslateBody {
    lang: String,
}

// --- Reading ----------------------------------------------------------------

/// `GET /v1/forum/themes`: every theme, with its question count (also the
/// choices of the « ask a question » form).
pub async fn themes(State(state): State<AppState>) -> Result<Response, ApiError> {
    let themes: Vec<Theme> = forum::themes(&state.db)
        .await?
        .into_iter()
        .map(Theme::from)
        .collect();
    Ok(no_store(StatusCode::OK, &Data { data: themes }))
}

/// `GET /v1/forum/themes/{slug}`.
pub async fn theme(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let theme = forum::theme(&state.db, &slug)
        .await?
        .ok_or_else(|| theme_not_found(&slug, uri.path()))?;
    Ok(no_store(
        StatusCode::OK,
        &Data {
            data: Theme::from(theme),
        },
    ))
}

/// `GET /v1/forum/questions`: pinned questions first, then by `sort`;
/// `theme` keeps one theme's questions.
pub async fn questions(
    State(state): State<AppState>,
    MaybeMember(reader): MaybeMember,
    RawQuery(query): RawQuery,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let mut params = Params::parse(query.as_deref());
    let theme = params.text("theme", 255);
    let sort = match params.one_of("sort", SORTS) {
        Some("popular") => Sort::Popular,
        Some("unanswered") => Sort::Unanswered,
        _ => Sort::Recent,
    };
    let limit = params.limit_or(PER_PAGE);
    let page = params.page(false).unwrap_or(1);
    params.finish()?;

    let theme_id = match &theme {
        Some(slug) => Some(
            forum::theme(&state.db, slug)
                .await?
                .ok_or_else(|| theme_not_found(slug, uri.path()))?
                .id,
        ),
        None => None,
    };
    let viewer = reader.as_ref().map(Member::id);
    let moderator = reader.as_ref().is_some_and(Member::is_moderator);
    let offset = i64::from(page - 1) * i64::from(limit);
    let (rows, total) = tokio::try_join!(
        forum::questions(
            &state.db,
            viewer,
            theme_id,
            sort,
            i64::from(limit),
            offset,
            EXCERPT_SOURCE_CHARS,
        ),
        forum::count_questions(&state.db, theme_id, sort),
    )?;
    let page = Page {
        data: rows
            .into_iter()
            .map(|row| to_summary(row, moderator))
            .collect(),
        next_cursor: None,
        total: Some(total),
    };
    Ok(no_store(StatusCode::OK, &page))
}

/// `GET /v1/forum/questions/{slug}`: the question and its answers, as a
/// tree. Counts a view once per reader (member, or else address) every two
/// hours.
pub async fn question(
    State(state): State<AppState>,
    MaybeMember(reader): MaybeMember,
    caller: Caller,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let viewer = reader.as_ref().map(Member::id);
    let mut row = forum::question(&state.db, viewer, &slug)
        .await?
        .ok_or_else(|| question_not_found(&slug, uri.path()))?;

    let who = who(reader.as_ref(), &caller);
    let decision = rate_limit::attempt(
        &state.redis,
        "forum-view",
        &format!("{}|{who}", row.id),
        1,
        VIEW_WINDOW_SECS,
    )
    .await;
    // Without Redis every view counts: the number is only indicative.
    if decision.is_none_or(|d| d.allowed()) {
        forum::add_view(&state.db, row.id).await?;
        row.views += 1;
    }

    let answers = forum::answers(&state.db, viewer, row.id).await?;
    let moderator = reader.as_ref().is_some_and(Member::is_moderator);
    let viewer = Viewer {
        is_author: viewer.is_some() && viewer == row.user_id,
        can_participate: reader.as_ref().is_some_and(|m| !m.is_muted()) && !row.locked,
        is_moderator: moderator,
    };
    let thread = to_detail(
        row,
        tree(&answers, None, moderator),
        answers.len(),
        viewer,
        moderator,
    );
    Ok(no_store(StatusCode::OK, &Data { data: thread }))
}

// --- Writing ----------------------------------------------------------------

/// `POST /v1/forum/questions`.
pub async fn ask(
    State(state): State<AppState>,
    Verified(member): Verified,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<AskBody>,
) -> Result<Response, ApiError> {
    member.require_unmuted(uri.path())?;
    let (theme, title, text) = (body.theme.trim(), body.title.trim(), body.body.trim());
    let mut errors = Vec::new();
    let theme = if theme.is_empty() {
        errors.push(ParameterError::field("theme", "required", "is required"));
        None
    } else {
        let found = forum::theme(&state.db, theme).await?;
        if found.is_none() {
            errors.push(ParameterError::field(
                "theme",
                "invalid",
                "is not the slug of a theme",
            ));
        }
        found
    };
    check_length(&mut errors, "title", title, 8, 150);
    check_length(&mut errors, "body", text, 15, 10_000);
    let Some(theme) = theme.filter(|_| errors.is_empty()) else {
        return Err(ApiError::InvalidFields(errors));
    };

    // A short random suffix: the address cannot be guessed, and two titles
    // alike do not collide (Laravel's `slugUnique`).
    let base = slug_base(title);
    for _ in 0..5 {
        let slug = format!("{base}-{}", random_suffix()?);
        if let Some(id) =
            forum::create_question(&state.db, theme.id, member.id(), title, &slug, text).await?
        {
            return Ok(no_store(
                StatusCode::CREATED,
                &Data {
                    data: Created {
                        id,
                        slug: Some(slug),
                    },
                },
            ));
        }
    }
    Err(ApiError::Internal(
        "no free slug for a question after 5 draws".to_owned(),
    ))
}

/// `POST /v1/forum/questions/{question}/answers` (id or slug): an answer, or a reply to an
/// answer of the same question.
pub async fn answer(
    State(state): State<AppState>,
    Verified(member): Verified,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<AnswerBody>,
) -> Result<Response, ApiError> {
    let question = find_question(&state, &slug, uri.path()).await?;
    member.require_unmuted(uri.path())?;
    if question.locked {
        return Err(ApiError::Conflict {
            reason: "locked",
            detail: "This thread is locked: it takes no more answers.".to_owned(),
        });
    }
    let text = body.body.trim();
    let mut errors = Vec::new();
    check_length(&mut errors, "body", text, 2, 10_000);
    if let Some(parent) = body.parent_id {
        // A reply hangs under an answer of the same question, never under
        // another question's thread.
        let same_question = forum::answer_ref(&state.db, parent)
            .await?
            .is_some_and(|p| p.question_id == question.id);
        if !same_question {
            errors.push(ParameterError::field(
                "parent_id",
                "invalid",
                "is not an answer of this question",
            ));
        }
    }
    if !errors.is_empty() {
        return Err(ApiError::InvalidFields(errors));
    }
    let id =
        forum::create_answer(&state.db, question.id, member.id(), body.parent_id, text).await?;
    Ok(no_store(
        StatusCode::CREATED,
        &Data {
            data: Created { id, slug: None },
        },
    ))
}

/// `POST /v1/forum/questions/{question}/answers/{id}/accept`: the question's
/// author marks an answer as accepted, or takes the mark back.
pub async fn accept(
    State(state): State<AppState>,
    Verified(member): Verified,
    Path((slug, answer)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let question = find_question(&state, &slug, uri.path()).await?;
    let answer = match crate::auth::user_id(&answer) {
        Some(id) => forum::answer_ref(&state.db, id).await?,
        None => None,
    }
    .ok_or_else(|| answer_not_found(uri.path()))?;
    if question.user_id != Some(member.id()) {
        return Err(ApiError::Forbidden {
            detail: "Only the author of the question accepts an answer.".to_owned(),
            instance: Some(uri.path().to_owned()),
        });
    }
    if answer.question_id != question.id {
        return Err(answer_not_found(uri.path()));
    }
    let accepted = forum::toggle_accepted(&state.db, question.id, answer.id).await?;
    Ok(no_store(
        StatusCode::OK,
        &Data {
            data: Accepted {
                accepted_answer_id: accepted,
            },
        },
    ))
}

/// `POST /v1/forum/questions/{question}/vote`: votes for a question, or
/// takes the vote back.
pub async fn vote_question(
    State(state): State<AppState>,
    Verified(member): Verified,
    Path(key): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let question = find_question(&state, &key, uri.path()).await?;
    toggle_vote(&state, &member, Target::Question, question.id).await
}

/// `POST /v1/forum/answers/{id}/vote`.
pub async fn vote_answer(
    State(state): State<AppState>,
    Verified(member): Verified,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let answer = match crate::auth::user_id(&id) {
        Some(id) => forum::answer_ref(&state.db, id).await?,
        None => None,
    }
    .ok_or_else(|| answer_not_found(uri.path()))?;
    toggle_vote(&state, &member, Target::Answer, answer.id).await
}

async fn toggle_vote(
    state: &AppState,
    member: &Member,
    target: Target,
    id: i64,
) -> Result<Response, ApiError> {
    let (voted, votes) = forum::toggle_vote(&state.db, member.id(), target, id).await?;
    Ok(no_store(
        StatusCode::OK,
        &Data {
            data: Vote { voted, votes },
        },
    ))
}

/// `POST /v1/forum/reports`: reports a question or an answer to the
/// moderators. 20 a minute per member.
pub async fn report(
    State(state): State<AppState>,
    Verified(member): Verified,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<ReportBody>,
) -> Result<Response, ApiError> {
    limit(
        &state,
        "forum-report",
        &member.id().to_string(),
        REPORT_LIMIT,
        rate_limit::WINDOW_SECS,
    )
    .await?;
    let mut errors = Vec::new();
    let target = match target_fields(
        &body.target_type,
        body.target_id,
        "target_type",
        "target_id",
    ) {
        Ok(target) => Some(target),
        Err(ApiError::InvalidFields(mut fields)) => {
            errors.append(&mut fields);
            None
        }
        Err(error) => return Err(error),
    };
    let kind = body.kind.trim();
    if kind.is_empty() {
        errors.push(ParameterError::field("type", "required", "is required"));
    } else if !REPORT_TYPES.contains(&kind) {
        errors.push(ParameterError::field(
            "type",
            "invalid",
            format!("must be one of: {}", REPORT_TYPES.join(", ")),
        ));
    }
    let message = body.message.as_deref().map(str::trim).unwrap_or_default();
    if message.chars().count() > 2000 {
        errors.push(ParameterError::field(
            "message",
            "too_long",
            "must be at most 2000 characters",
        ));
    }
    let Some((target, id)) = target.filter(|_| errors.is_empty()) else {
        return Err(ApiError::InvalidFields(errors));
    };
    if !exists(&state, target, id).await? {
        return Err(target_not_found(target, uri.path()));
    }
    let report = forum::create_report(&state.db, member.id(), target, id, kind, message).await?;
    Ok(no_store(
        StatusCode::CREATED,
        &Data {
            data: Created {
                id: report,
                slug: None,
            },
        },
    ))
}

// --- Translation --------------------------------------------------------------

/// `POST /v1/forum/questions/{question}/translations`: the title and body of a
/// question, translated by DeepL into `lang`.
pub async fn translate_question(
    State(state): State<AppState>,
    MaybeMember(reader): MaybeMember,
    caller: Caller,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<TranslateBody>,
) -> Result<Response, ApiError> {
    limit_translations(&state, reader.as_ref(), &caller).await?;
    let question = find_question(&state, &slug, uri.path()).await?;
    let texts = [question.title.as_str(), question.body.as_str()];
    let post = Post {
        target: Target::Question,
        id: question.id,
        hidden: question.hidden,
        texts: &texts,
    };
    translate(&state, reader.as_ref(), uri.path(), post, &body.lang).await
}

/// `POST /v1/forum/answers/{id}/translations`: the body of an answer,
/// translated.
pub async fn translate_answer(
    State(state): State<AppState>,
    MaybeMember(reader): MaybeMember,
    caller: Caller,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<TranslateBody>,
) -> Result<Response, ApiError> {
    limit_translations(&state, reader.as_ref(), &caller).await?;
    let answer = match crate::auth::user_id(&id) {
        Some(id) => forum::answer_ref(&state.db, id).await?,
        None => None,
    }
    .ok_or_else(|| answer_not_found(uri.path()))?;
    let texts = [answer.body.as_str()];
    let post = Post {
        target: Target::Answer,
        id: answer.id,
        hidden: answer.hidden,
        texts: &texts,
    };
    translate(&state, reader.as_ref(), uri.path(), post, &body.lang).await
}

/// 10 translations a minute per reader, counted before anything else, as
/// Laravel's `throttle:10,1`.
async fn limit_translations(
    state: &AppState,
    reader: Option<&Member>,
    caller: &Caller,
) -> Result<(), ApiError> {
    limit(
        state,
        "forum-translate",
        &who(reader, caller),
        TRANSLATE_LIMIT,
        rate_limit::WINDOW_SECS,
    )
    .await
}

/// A post to translate: a question's title and body, or an answer's body.
struct Post<'a> {
    target: Target,
    id: i64,
    hidden: bool,
    texts: &'a [&'a str],
}

/// Laravel's `TraducteurForum`: the stored translation while the post has
/// not changed (same SHA-256 of its texts), DeepL otherwise.
async fn translate(
    state: &AppState,
    reader: Option<&Member>,
    path: &str,
    post: Post<'_>,
    lang: &str,
) -> Result<Response, ApiError> {
    let lang = lang.trim();
    if lang.is_empty() {
        return Err(ApiError::InvalidFields(vec![ParameterError::field(
            "lang",
            "required",
            "is required",
        )]));
    }
    if !TRANSLATION_LANGS.contains(&lang) {
        return Err(ApiError::InvalidFields(vec![ParameterError::field(
            "lang",
            "invalid",
            format!("must be one of: {}", TRANSLATION_LANGS.join(", ")),
        )]));
    }
    // What moderation hid is not given away through a translation.
    if post.hidden && !reader.is_some_and(Member::is_moderator) {
        return Err(ApiError::Forbidden {
            detail: "This message is hidden by moderation.".to_owned(),
            instance: Some(path.to_owned()),
        });
    }
    let (config, target_lang) = match state
        .config
        .deepl
        .as_ref()
        .and_then(|c| c.target(lang).map(|t| (c, t)))
    {
        Some(found) => found,
        None => {
            return Err(ApiError::ServiceUnavailable {
                reason: "not_configured",
                detail: "The translation service is not configured.".to_owned(),
            });
        }
    };

    let hash = source_hash(post.texts);
    if let Some(stored) = forum::translation(&state.db, post.target, post.id, lang).await?
        && stored.source_hash == hash
    {
        return Ok(translated(stored, true));
    }
    let mut translations = deepl::translate(config, post.texts, target_lang)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "forum translation failed");
            ApiError::ServiceUnavailable {
                reason: "upstream_failed",
                detail: match error {
                    deepl::Error::Unavailable(_) => {
                        "The translation service is temporarily unavailable."
                    }
                    deepl::Error::Invalid(_) => "The translation service gave an invalid answer.",
                }
                .to_owned(),
            }
        })?;
    let source_lang = translations.first().and_then(|t| t.source_lang.clone());
    let body = translations.pop().map(|t| t.text).unwrap_or_default();
    let title = match post.target {
        Target::Question => translations.pop().map(|t| t.text),
        Target::Answer => None,
    };
    let row = TranslationRow {
        source_lang,
        target_lang: lang.to_owned(),
        source_hash: hash,
        title,
        body,
    };
    forum::save_translation(&state.db, post.target, post.id, &row).await?;
    Ok(translated(row, false))
}

fn translated(row: TranslationRow, cached: bool) -> Response {
    no_store(
        StatusCode::OK,
        &Data {
            data: Translation {
                title: row.title,
                body_md: row.body,
                source_lang: row.source_lang,
                target_lang: row.target_lang,
                cached,
            },
        },
    )
}

/// SHA-256 of the texts joined by NUL, in hex, as Laravel computed it.
fn source_hash(texts: &[&str]) -> String {
    let digest = Sha256::digest(texts.join("\0").as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

// --- Helpers ----------------------------------------------------------------

/// The question a write route names, by id or by slug. A slug always
/// holds a `-` and letters, so a number is an id.
async fn find_question(
    state: &AppState,
    key: &str,
    path: &str,
) -> Result<forum::QuestionRef, ApiError> {
    match crate::auth::user_id(key) {
        Some(id) => forum::question_ref_by_id(&state.db, id).await?,
        None => forum::question_ref(&state.db, key).await?,
    }
    .ok_or_else(|| question_not_found(key, path))
}

/// Who a per-reader limit counts: the member, or else the address.
fn who(reader: Option<&Member>, caller: &Caller) -> String {
    match reader {
        Some(member) => format!("user:{}", member.id()),
        None => format!("ip:{}", caller.ip.as_deref().unwrap_or("unknown")),
    }
}

/// `type`/`id` of a vote or a report: `question` or `answer`, and an id.
fn target_fields(
    kind: &str,
    id: Option<i64>,
    kind_field: &str,
    id_field: &str,
) -> Result<(Target, i64), ApiError> {
    let mut errors = Vec::new();
    let target = if kind.is_empty() {
        errors.push(ParameterError::field(kind_field, "required", "is required"));
        None
    } else {
        let target = Target::parse(kind);
        if target.is_none() {
            errors.push(ParameterError::field(
                kind_field,
                "invalid",
                "must be one of: question, answer",
            ));
        }
        target
    };
    if id.is_none() {
        errors.push(ParameterError::field(id_field, "required", "is required"));
    }
    match (target, id) {
        (Some(target), Some(id)) if errors.is_empty() => Ok((target, id)),
        _ => Err(ApiError::InvalidFields(errors)),
    }
}

async fn exists(state: &AppState, target: Target, id: i64) -> Result<bool, ApiError> {
    Ok(match target {
        Target::Question => forum::question_ref_by_id(&state.db, id).await?.is_some(),
        Target::Answer => forum::answer_ref(&state.db, id).await?.is_some(),
    })
}

/// Laravel's `required|string|min|max`, on the trimmed value.
fn check_length(
    errors: &mut Vec<ParameterError>,
    field: &str,
    value: &str,
    min: usize,
    max: usize,
) {
    let chars = value.chars().count();
    if chars == 0 {
        errors.push(ParameterError::field(field, "required", "is required"));
    } else if chars < min {
        errors.push(ParameterError::field(
            field,
            "too_short",
            format!("must be at least {min} characters"),
        ));
    } else if chars > max {
        errors.push(ParameterError::field(
            field,
            "too_long",
            format!("must be at most {max} characters"),
        ));
    }
}

fn theme_not_found(slug: &str, path: &str) -> ApiError {
    ApiError::NotFound {
        detail: format!("No forum theme has the slug '{slug}'."),
        instance: Some(path.to_owned()),
    }
}

fn question_not_found(slug: &str, path: &str) -> ApiError {
    ApiError::NotFound {
        detail: format!("No question has the slug or id '{slug}'."),
        instance: Some(path.to_owned()),
    }
}

pub(crate) fn answer_not_found(path: &str) -> ApiError {
    ApiError::NotFound {
        detail: "No such answer.".to_owned(),
        instance: Some(path.to_owned()),
    }
}

fn target_not_found(target: Target, path: &str) -> ApiError {
    ApiError::NotFound {
        detail: format!("No such {}.", target.morph()),
        instance: Some(path.to_owned()),
    }
}

fn date(at: Option<DateTime<Utc>>) -> Option<String> {
    at.map(timestamp)
}

fn to_summary(row: QuestionRow, moderator: bool) -> QuestionSummary {
    let (author, author_id) = author(row.user_id, row.author_pseudo);
    QuestionSummary {
        excerpt: (!row.hidden || moderator).then(|| excerpt(&row.body)),
        id: row.id,
        slug: row.slug,
        title: row.title,
        theme: ThemeRef {
            slug: row.theme_slug,
            name: row.theme_name,
            color: row.theme_color,
        },
        author,
        author_id,
        votes: row.votes,
        answer_count: row.answer_count,
        views: row.views,
        solved: row.accepted_answer_id.is_some(),
        has_voted: row.voted,
        is_hidden: row.hidden,
        is_locked: row.locked,
        is_pinned: row.pinned,
        created_at: date(row.created_at),
        updated_at: date(row.updated_at),
    }
}

fn to_detail(
    row: QuestionRow,
    answers: Vec<AnswerNode>,
    answer_count: usize,
    viewer: Viewer,
    moderator: bool,
) -> QuestionDetail {
    let (author, author_id) = author(row.user_id, row.author_pseudo);
    QuestionDetail {
        body_md: (!row.hidden || moderator).then_some(row.body),
        id: row.id,
        slug: row.slug,
        title: row.title,
        theme: ThemeRef {
            slug: row.theme_slug,
            name: row.theme_name,
            color: row.theme_color,
        },
        author,
        author_id,
        votes: row.votes,
        views: row.views,
        has_voted: row.voted,
        accepted_answer_id: row.accepted_answer_id,
        is_hidden: row.hidden,
        hidden_reason: row.hidden_reason,
        is_locked: row.locked,
        is_pinned: row.pinned,
        created_at: date(row.created_at),
        updated_at: date(row.updated_at),
        answer_count,
        answers,
        viewer,
    }
}

/// The replies to `parent`, each with its own replies. A reply whose parent
/// was deleted is left out with it, as in Laravel.
fn tree(rows: &[AnswerRow], parent: Option<i64>, moderator: bool) -> Vec<AnswerNode> {
    rows.iter()
        .filter(|row| row.parent_id == parent)
        .map(|row| {
            let (author, author_id) = author(row.user_id, row.author_pseudo.clone());
            AnswerNode {
                id: row.id,
                body_md: (!row.hidden || moderator).then(|| row.body.clone()),
                author,
                author_id,
                votes: row.votes,
                has_voted: row.voted,
                is_hidden: row.hidden,
                hidden_reason: row.hidden_reason.clone(),
                created_at: date(row.created_at),
                children: tree(rows, Some(row.id), moderator),
            }
        })
        .collect()
}

/// Laravel's `Str::limit(strip_tags($body), 160)`.
fn excerpt(body: &str) -> String {
    let text = strip_tags(body);
    if text.chars().count() <= EXCERPT_CHARS {
        return text;
    }
    let cut: String = text.chars().take(EXCERPT_CHARS).collect();
    format!("{}...", cut.trim_end())
}

/// Removes HTML tags and comments, as PHP's `strip_tags`: a `<` followed by
/// a letter, `/`, `!` or `?` opens a tag up to the next `>`; any other `<`
/// (`a < b`) is text.
fn strip_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let opens = c == '<'
            && chars
                .peek()
                .is_some_and(|n| n.is_alphabetic() || matches!(n, '/' | '!' | '?'));
        if opens {
            for skipped in chars.by_ref() {
                if skipped == '>' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The slug of a title without its suffix: its first 60 characters in
/// lower-case ASCII, words joined by `-` (Laravel's `Str::slug`), or
/// `question` when nothing is left.
fn slug_base(title: &str) -> String {
    let cut: String = title.chars().take(60).collect();
    let mut ascii = String::with_capacity(cut.len());
    for c in cut.trim_end().chars() {
        match c {
            '@' => ascii.push_str(" at "),
            c if c.is_ascii() => ascii.push(c.to_ascii_lowercase()),
            c => ascii.push_str(transliterate(c)),
        }
    }
    let mut slug = String::with_capacity(ascii.len());
    for c in ascii.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        "question".to_owned()
    } else {
        slug.to_owned()
    }
}

/// Lower-case ASCII for the letters of French, the other Latin alphabets
/// near it, and Russian; nothing for the rest.
fn transliterate(c: char) -> &'static str {
    let lower: Vec<char> = c.to_lowercase().collect();
    let c = match lower.as_slice() {
        [single] => *single,
        _ => c,
    };
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ą' | 'ă' => "a",
        'æ' => "ae",
        'ç' | 'ć' | 'č' => "c",
        'ď' | 'đ' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' | 'ě' => "e",
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'ı' => "i",
        'ł' | 'ľ' => "l",
        'ñ' | 'ń' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => "o",
        'œ' => "oe",
        'ř' => "r",
        'ś' | 'š' | 'ş' => "s",
        'ß' => "ss",
        'ť' | 'ţ' => "t",
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => "u",
        'ý' | 'ÿ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        'а' => "a",
        'б' => "b",
        'в' => "v",
        'г' => "g",
        'д' => "d",
        'е' => "e",
        'ё' => "yo",
        'ж' => "zh",
        'з' => "z",
        'и' => "i",
        'й' => "y",
        'к' => "k",
        'л' => "l",
        'м' => "m",
        'н' => "n",
        'о' => "o",
        'п' => "p",
        'р' => "r",
        'с' => "s",
        'т' => "t",
        'у' => "u",
        'ф' => "f",
        'х' => "h",
        'ц' => "ts",
        'ч' => "ch",
        'ш' => "sh",
        'щ' => "shch",
        'ъ' | 'ь' => "",
        'ы' => "y",
        'э' => "e",
        'ю' => "yu",
        'я' => "ya",
        _ => " ",
    }
}

/// Six random characters among `a-z0-9`.
fn random_suffix() -> Result<String, ApiError> {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut bytes = [0u8; 6];
    getrandom::fill(&mut bytes)
        .map_err(|e| ApiError::Internal(format!("no randomness for a slug: {e}")))?;
    Ok(bytes
        .iter()
        .map(|b| ALPHABET[usize::from(*b) % ALPHABET.len()] as char)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_follow_laravel() {
        assert_eq!(
            slug_base("Comment factoriser un trinôme ?"),
            "comment-factoriser-un-trinome"
        );
        assert_eq!(slug_base("L'œuf & la poule"), "l-oeuf-la-poule");
        assert_eq!(slug_base("Что такое интеграл?"), "chto-takoe-integral");
        assert_eq!(slug_base("mail@site"), "mail-at-site");
        assert_eq!(slug_base("???"), "question");
        assert_eq!(slug_base("数学"), "question");
        let long = "a".repeat(80);
        assert_eq!(slug_base(&long).len(), 60);
        let suffix = random_suffix().unwrap();
        assert_eq!(suffix.len(), 6);
        assert!(
            suffix
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        );
    }

    #[test]
    fn excerpts_are_plain_text_of_160_characters() {
        assert_eq!(excerpt("Un <b>gras</b> et a < b"), "Un gras et a < b");
        assert_eq!(excerpt("<!-- note -->Texte"), "Texte");
        let long = format!("{} fin", "é".repeat(159));
        let cut = excerpt(&long);
        assert_eq!(cut, format!("{}...", "é".repeat(159)));
        assert_eq!(excerpt("court"), "court");
    }

    #[test]
    fn the_source_hash_is_laravels() {
        // hash('sha256', "a\0b") in PHP.
        assert_eq!(
            source_hash(&["a", "b"]),
            "59b271ae1bbcb1d31d41929817f4b16fb439eb4f31520b5ad1d5ce98920a7138"
        );
        assert_eq!(source_hash(&["x"]).len(), 64);
    }

    #[test]
    fn replies_hang_under_their_parent() {
        let row = |id, parent_id, hidden| AnswerRow {
            id,
            parent_id,
            body: format!("body {id}"),
            user_id: None,
            author_pseudo: None,
            votes: 0,
            voted: false,
            created_at: None,
            hidden,
            hidden_reason: None,
        };
        // 4 replies to 3, which was deleted: it is not in the rows.
        let rows = vec![
            row(1, None, false),
            row(2, Some(1), true),
            row(4, Some(3), false),
        ];
        let nodes = tree(&rows, None, false);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].children[0].id, 2);
        assert_eq!(nodes[0].children[0].body_md, None);
        assert_eq!(
            tree(&rows, None, true)[0].children[0].body_md.as_deref(),
            Some("body 2")
        );
    }
}
