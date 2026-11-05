use axum::{
    extract::Request,
    http::{header::HOST, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

/// O servidor escuta so em loopback, mas isso nao basta contra DNS rebinding:
/// uma pagina em `http://rebind.atacante.com:3001` cujo DNS passa a apontar para
/// 127.0.0.1 fica same-origin com a API e pode chamar login/register/recover.
/// O navegador ainda manda o nome do atacante no `Host`, entao basta aceitar
/// apenas nomes de loopback.
pub fn is_loopback_host(host: &str) -> bool {
    let name = match host.strip_prefix('[') {
        // IPv6 literal: "[::1]:3001" -> "::1"
        Some(rest) => match rest.split_once(']') {
            Some((ip, port)) if port.is_empty() || port.starts_with(':') => ip,
            _ => return false,
        },
        None => host.rsplit_once(':').map_or(host, |(name, _port)| name),
    };
    let name = name.to_ascii_lowercase();
    matches!(name.as_str(), "localhost" | "127.0.0.1" | "::1" | "tauri.localhost")
}

pub async fn require_loopback_host(request: Request, next: Next) -> Response {
    let host = request
        .headers()
        .get(HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !is_loopback_host(host) {
        tracing::warn!("[Host] Requisicao recusada com Host nao-loopback: {:?}", host);
        return (StatusCode::MISDIRECTED_REQUEST, "Host nao permitido.").into_response();
    }
    next.run(request).await
}

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

    /// As duas politicas do projeto tem de fechar as mesmas portas.
    ///
    /// O produto instalado e uma webview: quem vale la e a CSP do `tauri.conf.json`,
    /// nao a que este middleware envia. Enquanto so a do servidor era testada, a da
    /// webview carregava `script-src 'unsafe-inline'` e ninguem percebia — a defesa
    /// efetiva era a mais fraca das duas, exatamente onde o usuario final executa.
    #[test]
    fn the_webview_csp_is_no_weaker_than_the_server_csp() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json");
        let csp = conf["app"]["security"]["csp"]
            .as_str()
            .expect("app.security.csp precisa existir");

        for exigido in [
            "default-src 'self'",
            "object-src 'none'",
            "base-uri 'none'",
            "form-action 'none'",
            "frame-ancestors 'none'",
        ] {
            assert!(csp.contains(exigido), "CSP da webview sem {exigido:?}: {csp}");
        }

        assert!(
            !csp.contains("unsafe-eval"),
            "a webview nao pode permitir unsafe-eval: {csp}"
        );

        // `style-src 'unsafe-inline'` fica: o React aplica estilos via `style={{}}`.
        // Em `script-src` nao ha justificativa, e e o que torna um XSS executavel.
        let script_src = csp
            .split(';')
            .map(str::trim)
            .find(|d| d.starts_with("script-src"))
            .expect("script-src precisa ser declarado");
        assert!(
            !script_src.contains("unsafe-inline"),
            "script-src da webview nao pode permitir unsafe-inline: {script_src}"
        );
    }

    /// As fontes sao servidas do bundle. Um `@import` de fonts.googleapis.com faria
    /// o app chamar um terceiro a cada abertura — e so funcionaria se a CSP fosse
    /// reaberta, entao travar a CSP e o que impede a volta silenciosa.
    #[test]
    fn the_webview_csp_does_not_allow_external_font_hosts() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let csp = conf["app"]["security"]["csp"].as_str().unwrap();

        assert!(!csp.contains("fonts.googleapis.com"), "CSP ainda libera o Google Fonts");
        assert!(!csp.contains("fonts.gstatic.com"), "CSP ainda libera o gstatic");
    }

    #[test]
    fn accepts_only_loopback_hosts() {
        use super::is_loopback_host;
        for ok in ["localhost:3001", "127.0.0.1:3001", "[::1]:3001", "localhost", "LOCALHOST:3001", "tauri.localhost"] {
            assert!(is_loopback_host(ok), "{} deveria ser aceito", ok);
        }
        for bad in [
            "",
            "rebind.atacante.com:3001",
            "evil.com",
            "localhost.evil.com:3001",
            "127.0.0.1.nip.io:3001",
            "192.168.0.10:3001",
            "[::1]evil:3001",
        ] {
            assert!(!is_loopback_host(bad), "{:?} deveria ser recusado", bad);
        }
    }
}
