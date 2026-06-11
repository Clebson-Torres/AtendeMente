use axum::{
    extract::Request,
    http::HeaderValue,
    middleware::Next,
    response::Response,
};

pub async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;

    response
        .headers_mut()
        .insert("X-Frame-Options", HeaderValue::from_static("DENY"));
    response
        .headers_mut()
        .insert("X-Content-Type-Options", HeaderValue::from_static("nosniff"));
    response.headers_mut().insert(
        "Referrer-Policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    response.headers_mut().insert(
        "Permissions-Policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=()"),
    );

    response
}
