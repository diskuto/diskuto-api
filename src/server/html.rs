//! Endpoints that serve plain HTML web pages.

use actix_web::{web::{self, get, route, ServiceConfig}, HttpResponse};

use super::Error;


pub fn routes(cfg: &mut ServiceConfig) {
    cfg.service(
        web::resource("/")
        .route(get().to(redirect_to_api))
    );

    cfg.service(
        web::resource("/diskuto/")
        .route(get().to(api_docs_link))
    );

    cfg.default_service(route().to(not_found));
}

pub async fn not_found() -> Result<HttpResponse, Error> {
    Ok(HttpResponse::NotFound().body("404 Not Found"))
}

/// If this page is requested, either the API server is running standalone,
/// or the co-hosting with the web UI is incorrectly configured.
/// (So use a temp redirect.)
pub async fn redirect_to_api() -> HttpResponse {
    HttpResponse::TemporaryRedirect()
        .append_header(("Location", "/diskuto/"))
        .finish()
}

pub async fn api_docs_link() -> HttpResponse {
    let body = r#"
<html>
<head><title>Diskuto REST API</title></head>
<body>
    <h1>Diskuto REST API</h1>
    <p>See:
        <ul>
            <li>documentation <a href="https://github.com/diskuto/diskuto-api/">on GitHub</a></li>
            <li><a href="https://diskuto.github.io/diskuto-api/api/">SwaggerUI</a> API browser</li>
        </ul>
    </p>
</body>
</html>
    "#;

    HttpResponse::Ok()
        .append_header(("content-type", "text/html"))
        .body(body)
}