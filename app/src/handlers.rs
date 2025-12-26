use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Form,
};
use tracing::error;

use crate::{
    error::{AppError, HtmlTemplate},
    models::{Link, NewLink, SearchParams},
    state::AppState,
    templates::{CreateLinkTemplate, IndexTemplate, LinkRowTemplate, LinksListTemplate},
    utils::levenshtein,
};

pub async fn show_ui() -> impl IntoResponse {
    HtmlTemplate(IndexTemplate)
}

pub async fn redirect_link(
    State(state): State<AppState>,
    AxumPath(short_link): AxumPath<String>,
) -> Result<Response, AppError> {
    let link: Option<Link> =
        sqlx::query_as("SELECT short_link, url, created_at FROM links WHERE short_link = ?")
            .bind(&short_link)
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| {
                AppError(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    anyhow::anyhow!("Database error"),
                )
            })?;

    match link {
        Some(l) => Ok(Redirect::to(&l.url).into_response()),
        None => Ok(HtmlTemplate(CreateLinkTemplate { short_link }).into_response()),
    }
}

pub async fn list_links(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = params.limit;
    let offset = (params.page - 1) * limit;
    let q = params.q.clone().unwrap_or_default();
    let q_trim = q.trim();

    let links = if !q_trim.is_empty() {
        // Advanced Fuzzy Search
        // 1. If short (<=3 chars), use standard prefix/trigram match
        // 2. If long, break into trigrams and OR them to find candidates

        let query_str = if q_trim.chars().count() <= 3 {
            format!("{}", q_trim)
        } else {
            // Generate trigrams: "kubernetes" -> "kub" OR "ube" OR "ber" ...
            let chars: Vec<char> = q_trim.chars().collect();
            let mut trigrams = Vec::new();
            for i in 0..chars.len().saturating_sub(2) {
                let trigram: String = chars[i..i + 3].iter().collect();
                // Escape double quotes just in case
                let safe_tri = trigram.replace("\"", "\"\"");
                trigrams.push(format!("\"{}\"", safe_tri));
            }
            if trigrams.is_empty() {
                format!("{}", q_trim)
            } else {
                trigrams.join(" OR ")
            }
        };

        // Fetch MORE candidates than the limit to allow re-sorting
        // We'll fetch 4x the limit to have a good pool of candidates
        let candidate_limit = limit * 4;

        let mut candidates = sqlx::query_as::<_, Link>(
            "SELECT l.short_link, l.url, l.created_at 
             FROM links l
             JOIN links_fts f ON l.rowid = f.rowid
             WHERE links_fts MATCH ? 
             ORDER BY rank
             LIMIT ?",
        )
        .bind(query_str)
        .bind(candidate_limit)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| {
            error!("Search error: {:?}", e);
            AppError(
                StatusCode::INTERNAL_SERVER_ERROR,
                anyhow::anyhow!("Failed to fetch links"),
            )
        })?;

        // Refine sorting with Levenshtein in memory
        candidates.sort_by(|a, b| {
            let dist_a = levenshtein(&a.short_link, q_trim);
            let dist_b = levenshtein(&b.short_link, q_trim);
            dist_a.cmp(&dist_b)
        });

        // Apply pagination in memory since we re-sorted
        // (Note: this simple pagination approach resets scope to the fetched candidates)
        // For true deep pagination with sorting, we'd need to fetch all candidates,
        // but for a fuzzy search, usually only the top results matter.
        let start = offset as usize;
        let end = std::cmp::min(start + limit as usize + 1, candidates.len());

        if start >= candidates.len() {
            Vec::new()
        } else {
            candidates[start..end].to_vec()
        }
    } else {
        sqlx::query_as::<_, Link>(
            "SELECT short_link, url, created_at FROM links ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(limit + 1) // Fetch one extra to check for next page
        .bind(offset)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| {
            error!("Search error: {:?}", e);
            AppError(
                StatusCode::INTERNAL_SERVER_ERROR,
                anyhow::anyhow!("Failed to fetch links"),
            )
        })?
    };

    let has_next = links.len() > limit as usize;
    let mut links = links; // make mutable
    if has_next {
        links.pop();
    }

    Ok(HtmlTemplate(LinksListTemplate {
        links,
        page: params.page,
        has_next,
        q,
    }))
}

pub async fn add_link(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(new_link): Form<NewLink>,
) -> Result<Response, AppError> {
    // Basic validation for short_link
    let is_valid = new_link
        .short_link
        .chars()
        .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == ':');
    if !is_valid || new_link.short_link.is_empty() {
        return Err(AppError(
            StatusCode::BAD_REQUEST,
            anyhow::anyhow!("Invalid characters in short link"),
        ));
    }

    sqlx::query("INSERT INTO links (short_link, url) VALUES (?, ?)")
        .bind(&new_link.short_link)
        .bind(&new_link.url)
        .execute(&state.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => AppError(
                StatusCode::CONFLICT,
                anyhow::anyhow!("Short link '{}' already exists", new_link.short_link),
            ),
            _ => AppError(
                StatusCode::INTERNAL_SERVER_ERROR,
                anyhow::anyhow!("Failed to create link"),
            ),
        })?;

    if headers.contains_key("hx-request") {
        let link: Link =
            sqlx::query_as("SELECT short_link, url, created_at FROM links WHERE short_link = ?")
                .bind(&new_link.short_link)
                .fetch_one(&state.pool)
                .await
                .map_err(|_|
                    AppError(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        anyhow::anyhow!("Failed to fetch created link"),
                    )
                )?;
        Ok(HtmlTemplate(LinkRowTemplate { link }).into_response())
    } else {
        Ok(Redirect::to("/link").into_response())
    }
}

pub async fn delete_link(
    State(state): State<AppState>,
    AxumPath(short_link): AxumPath<String>,
) -> Result<StatusCode, AppError> {
    let result = sqlx::query("DELETE FROM links WHERE short_link = ?")
        .bind(short_link)
        .execute(&state.pool)
        .await
        .map_err(|_|
            AppError(
                StatusCode::INTERNAL_SERVER_ERROR,
                anyhow::anyhow!("Failed to delete link"),
            )
        )?;

    if result.rows_affected() == 0 {
        return Err(AppError(
            StatusCode::NOT_FOUND,
            anyhow::anyhow!("Link not found"),
        ));
    }

    Ok(StatusCode::OK)
}
