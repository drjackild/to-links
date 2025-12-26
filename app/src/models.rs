use serde::Deserialize;

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Link {
    pub short_link: String,
    pub url: String,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Deserialize)]
pub struct NewLink {
    pub short_link: String,
    pub url: String,
}

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_page() -> u32 {
    1
}

fn default_limit() -> u32 {
    10
}
