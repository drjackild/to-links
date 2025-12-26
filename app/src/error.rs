use crate::templates::FormErrorTemplate;
use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use tracing::error;

pub struct AppError(pub StatusCode, pub anyhow::Error);

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

pub struct HtmlTemplate<T>(pub T);

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
