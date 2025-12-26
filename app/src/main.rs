mod error;
mod handlers;
mod models;
mod state;
mod templates;
mod utils;

use axum::{
    routing::{delete, get},
    Router,
};
use clap::Parser;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::{net::SocketAddr, str::FromStr};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    handlers::{add_link, delete_link, list_links, redirect_link, show_ui},
    state::AppState,
};

// --- CLI Configuration ---

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the SQLite database file
    #[arg(long)]
    db: Option<String>,
}

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