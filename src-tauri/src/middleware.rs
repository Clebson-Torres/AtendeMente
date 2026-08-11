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
    // Content-Security-Policy is set here as well as in tauri.conf.json: the
    // webview honours the config CSP, but a phone browsing over mobile access
    // only sees what the server sends. `connect-src 'self'` keeps a compromised
    // page from shipping patient data to a third party.
    response.headers_mut().insert(
        "Content-Security-Policy",
        HeaderValue::from_static(
            "default-src 'self'; \
             connect-src 'self'; \
             img-src 'self' data: blob:; \
             style-src 'self' 'unsafe-inline'; \
             font-src 'self' data:; \
             script-src 'self'; \
             object-src 'none'; \
             base-uri 'none'; \
             form-action 'none'; \
             frame-ancestors 'none'",
        ),
    );

    // Deliberately no Strict-Transport-Security: this server speaks plain HTTP
    // and binds loopback only, so HSTS would be ignored by the browser and would
    // only create the impression that the transport is protected. Add it together
    // with real TLS, not before.

    response
}
