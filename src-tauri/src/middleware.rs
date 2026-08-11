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

#[cfg(test)]
mod tests {
    use super::security_headers;
    use axum::body::Body;
    use axum::http::{Request, Response, StatusCode};
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    async fn respond() -> Response<Body> {
        let app = Router::new()
            .route("/x", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(security_headers));
        app.oneshot(Request::builder().uri("/x").body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn sets_the_expected_security_headers() {
        let res = respond().await;
        assert_eq!(res.status(), StatusCode::OK);
        let h = res.headers();

        assert_eq!(h["X-Frame-Options"], "DENY");
        assert_eq!(h["X-Content-Type-Options"], "nosniff");
        assert_eq!(h["Referrer-Policy"], "strict-origin-when-cross-origin");
        assert!(h["Permissions-Policy"].to_str().unwrap().contains("camera=()"));
    }

    #[tokio::test]
    async fn csp_locks_down_the_dangerous_directives() {
        let res = respond().await;
        let csp = res.headers()["Content-Security-Policy"].to_str().unwrap().to_string();

        // `connect-src 'self'` impede uma pagina comprometida de enviar prontuario
        // para terceiros; os demais fecham injecao de objeto/base/form e enquadramento.
        for exigido in [
            "default-src 'self'",
            "connect-src 'self'",
            "object-src 'none'",
            "base-uri 'none'",
            "form-action 'none'",
            "frame-ancestors 'none'",
        ] {
            assert!(csp.contains(exigido), "CSP sem {exigido:?}: {csp}");
        }
        assert!(
            !csp.contains("unsafe-eval"),
            "CSP nao deve permitir unsafe-eval: {csp}"
        );
    }

    /// HSTS over plain HTTP is ignored by browsers and only makes the transport
    /// look protected. It must stay absent until the server actually speaks TLS.
    #[tokio::test]
    async fn does_not_advertise_hsts_over_plain_http() {
        let res = respond().await;
        assert!(
            !res.headers().contains_key("Strict-Transport-Security"),
            "HSTS nao deve ser enviado enquanto o servidor fala HTTP puro"
        );
    }
}
