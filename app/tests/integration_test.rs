use app::{create_router, setup_db, state::AppState};
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn setup_app() -> axum::Router {
    let pool = setup_db("sqlite::memory:")
        .await
        .expect("Failed to create DB");
    let state = AppState { pool };
    create_router(state)
}

#[tokio::test]
async fn test_create_and_fuzzy_search() {
    let app = setup_app().await;

    // Create a Link
    let request = Request::builder()
        .uri("/api/links")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header("HX-Request", "true")
        .body(Body::from(
            "short_link=kubernetes&url=https://kubernetes.io",
        ))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Search with Typo
    let request = Request::builder()
        .uri("/api/links?q=kbernetes")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("kubernetes"));
}

#[tokio::test]
async fn test_redirection() {
    let app = setup_app().await;

    // 1. Create link
    let request = Request::builder()
        .uri("/api/links")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("short_link=google&url=https://google.com"))
        .unwrap();
    app.clone().oneshot(request).await.unwrap();

    // 2. Test redirect
    let request = Request::builder()
        .uri("/google")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers().get(header::LOCATION).unwrap(),
        "https://google.com"
    );
}

#[tokio::test]
async fn test_pagination() {
    let app = setup_app().await;

    // Create 25 links (default limit is 20)
    for i in 1..=25 {
        let request = Request::builder()
            .uri("/api/links")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!(
                "short_link=link{:02}&url=https://example.com/{}",
                i, i
            )))
            .unwrap();
        app.clone().oneshot(request).await.unwrap();
    }

    // Check page 1
    let request = Request::builder()
        .uri("/api/links?page=1")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    assert!(body_str.contains("Next"));
    assert!(body_str.contains("Page 1"));

    // Check page 2
    let request = Request::builder()
        .uri("/api/links?page=2")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    assert!(body_str.contains("Previous"));
    assert!(body_str.contains("Page 2"));
}

#[tokio::test]
async fn test_duplicate_error() {
    let app = setup_app().await;

    // 1. Create a link
    let request = Request::builder()
        .uri("/api/links")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("short_link=test&url=https://test.com"))
        .unwrap();
    app.clone().oneshot(request).await.unwrap();

    // 2. Try to create the same link
    let request = Request::builder()
        .uri("/api/links")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("short_link=test&url=https://other.com"))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();

    // Client error -> 200 with HX-Retarget in our AppError implementation
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("HX-Retarget").unwrap(),
        "#form-error"
    );

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("already exists"));
}

#[tokio::test]
async fn test_validation_error() {
    let app = setup_app().await;

    // Try to create link with invalid characters
    let request = Request::builder()
        .uri("/api/links")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("short_link=invalid spaces&url=https://test.com"))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("HX-Retarget").unwrap(),
        "#form-error"
    );

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("Invalid characters"));
}
