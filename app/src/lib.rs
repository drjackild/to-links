pub mod error;
pub mod handlers;
pub mod models;
pub mod state;
pub mod templates;
pub mod utils;

use crate::handlers::{add_link, delete_link, list_links, redirect_link, show_ui};
use crate::state::AppState;
use axum::{
    Router,
    routing::{delete, get},
};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::str::FromStr;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/link", get(show_ui))
        .route("/{short_link}", get(redirect_link))
        .route("/api/links", get(list_links).post(add_link))
        .route("/api/links/{short_link}", delete(delete_link))
        .with_state(state)
        .layer(tower_http::trace::TraceLayer::new_for_http())
}

pub async fn setup_db(db_path: &str) -> anyhow::Result<SqlitePool> {
    let db_options = SqliteConnectOptions::from_str(db_path)?.create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(db_options)
        .await?;

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

    Ok(pool)
}
