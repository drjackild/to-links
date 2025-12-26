use askama::Template;
use axum::{
    Form, Router,
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{delete, get},
};
use clap::Parser;
use serde::Deserialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::{net::SocketAddr, str::FromStr};
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// --- CLI Configuration ---

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the SQLite database file
    #[arg(long)]
    db: Option<String>,
}

// --- Database Models ---

#[derive(sqlx::FromRow, Debug, Clone)]
struct Link {
    short_link: String,
    url: String,
    created_at: chrono::NaiveDateTime,
}

#[derive(Deserialize)]
struct NewLink {
    short_link: String,
    url: String,
}

#[derive(Deserialize)]
struct SearchParams {
    q: Option<String>,
    #[serde(default = "default_page")]
    page: u32,
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_page() -> u32 {
    1
}

fn default_limit() -> u32 {
    10
}

// --- HTML Templates ---

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate;

#[derive(Template)]
#[template(path = "create_link.html")]
struct CreateLinkTemplate {
    short_link: String,
}

#[derive(Template)]
#[template(path = "links_list.html")]
struct LinksListTemplate {
    links: Vec<Link>,
    page: u32,
    has_next: bool,
    q: String,
}

#[derive(Template)]
#[template(path = "link_row.html")]
struct LinkRowTemplate {
    link: Link,
}

#[derive(Template)]
#[template(path = "form_error.html")]
struct FormErrorTemplate<'a> {
    message: &'a str,
}

// --- Application State ---

#[derive(Clone)]
struct AppState {
    pool: SqlitePool,
}

// --- Main Application ---

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "app=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Parse CLI arguments
    let args = Args::parse();

    // Determine database path
    let db_path = match args.db {
        Some(path) => path,
        None => {
            let home_dir =
                dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
            let default_path = home_dir.join(".local/share/to-links");
            std::fs::create_dir_all(&default_path)?;
            default_path.join("app.db").to_string_lossy().to_string()
        }
    };
    info!("Using database at: {}", db_path);

    // Set up database connection pool
    let db_options = SqliteConnectOptions::from_str(&db_path)?.create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(db_options)
        .await?;

    // Run database migrations
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS links (
            short_link TEXT PRIMARY KEY NOT NULL,
            url TEXT NOT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS links_fts USING fts5(short_link, url, content='links', content_rowid='rowid', tokenize='trigram');
        CREATE TRIGGER IF NOT EXISTS links_ai AFTER INSERT ON links BEGIN
            INSERT INTO links_fts(rowid, short_link, url) VALUES (new.rowid, new.short_link, new.url);
        END;
        CREATE TRIGGER IF NOT EXISTS links_ad AFTER DELETE ON links BEGIN
            INSERT INTO links_fts(links_fts, rowid, short_link, url) VALUES('delete', old.rowid, old.short_link, old.url);
        END;
        CREATE TRIGGER IF NOT EXISTS links_au AFTER UPDATE ON links BEGIN
            INSERT INTO links_fts(links_fts, rowid, short_link, url) VALUES('delete', old.rowid, old.short_link, old.url);
            INSERT INTO links_fts(rowid, short_link, url) VALUES (new.rowid, new.short_link, new.url);
        END;
        "#,
    )
    .execute(&pool)
    .await?;

    // Create application state
    let app_state = AppState { pool };

    // Build router
    let app = Router::new()
        .route("/link", get(show_ui))
        .route("/{short_link}", get(redirect_link))
        .route("/api/links", get(list_links).post(add_link))
        .route("/api/links/{short_link}", delete(delete_link))
        .with_state(app_state)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// --- Route Handlers ---

async fn show_ui() -> impl IntoResponse {
    HtmlTemplate(IndexTemplate)
}

async fn redirect_link(
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

fn levenshtein(a: &str, b: &str) -> usize {
    let len_a = a.chars().count();
    let len_b = b.chars().count();
    if len_a == 0 {
        return len_b;
    }
    if len_b == 0 {
        return len_a;
    }

    let mut dp = vec![vec![0; len_b + 1]; len_a + 1];

    for i in 0..=len_a {
        dp[i][0] = i;
    }
    for j in 0..=len_b {
        dp[0][j] = j;
    }

    for (i, ca) in a.chars().enumerate() {
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            dp[i + 1][j + 1] = std::cmp::min(
                dp[i][j + 1] + 1,
                std::cmp::min(dp[i + 1][j] + 1, dp[i][j] + cost),
            );
        }
    }
    dp[len_a][len_b]
}

async fn list_links(
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
            format!("\"{}\"", q_trim)
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
                format!("\"{}\"", q_trim)
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

async fn add_link(
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
                .map_err(|_| {
                    AppError(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        anyhow::anyhow!("Failed to fetch created link"),
                    )
                })?;
        Ok(HtmlTemplate(LinkRowTemplate { link }).into_response())
    } else {
        Ok(Redirect::to("/link").into_response())
    }
}

async fn delete_link(
    State(state): State<AppState>,
    AxumPath(short_link): AxumPath<String>,
) -> Result<StatusCode, AppError> {
    let result = sqlx::query("DELETE FROM links WHERE short_link = ?")
        .bind(short_link)
        .execute(&state.pool)
        .await
        .map_err(|_| {
            AppError(
                StatusCode::INTERNAL_SERVER_ERROR,
                anyhow::anyhow!("Failed to delete link"),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err(AppError(
            StatusCode::NOT_FOUND,
            anyhow::anyhow!("Link not found"),
        ));
    }

    Ok(StatusCode::OK)
}

// --- Error Handling ---

struct AppError(StatusCode, anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        error!("Error: {:?}", self.1);
        let message = self.1.to_string();
        let template = FormErrorTemplate { message: &message };
        // For client errors triggered by HTMX, we return 200 OK and use response headers
        // to indicate that it's an "error" to be handled by HTMX.
        // This avoids needing custom javascript to handle 4xx responses.
        let status = if self.0.is_client_error() {
            StatusCode::OK
        } else {
            self.0
        };
        let mut res = (status, HtmlTemplate(template)).into_response();
        res.headers_mut()
            .insert("HX-Retarget", "#form-error".parse().unwrap());
        res.headers_mut()
            .insert("HX-Reswap", "innerHTML".parse().unwrap());
        res
    }
}

// --- Askama Axum Integration ---

struct HtmlTemplate<T>(T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => Html(html).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template. Error: {}", e),
            )
                .into_response(),
        }
    }
}
