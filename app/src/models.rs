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

impl NewLink {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.short_link.is_empty() {
            return Err("Short link cannot be empty");
        }
        let is_valid = self
            .short_link
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == ':');
        if !is_valid {
            return Err("Invalid characters in short link");
        }
        Ok(())
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_link_validation() {
        let valid_link = NewLink {
            short_link: "my-link".to_string(),
            url: "https://example.com".to_string(),
        };
        assert!(valid_link.validate().is_ok());

        let valid_link_dots = NewLink {
            short_link: "my.link".to_string(),
            url: "https://example.com".to_string(),
        };
        assert!(valid_link_dots.validate().is_ok());

        let invalid_chars = NewLink {
            short_link: "my link".to_string(), // space is invalid
            url: "https://example.com".to_string(),
        };
        assert!(invalid_chars.validate().is_err());

        let empty_link = NewLink {
            short_link: "".to_string(),
            url: "https://example.com".to_string(),
        };
        assert!(empty_link.validate().is_err());

        let invalid_symbol = NewLink {
            short_link: "link!".to_string(),
            url: "https://example.com".to_string(),
        };
        assert!(invalid_symbol.validate().is_err());
    }
}
