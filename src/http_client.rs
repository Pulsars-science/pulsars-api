//! The outgoing HTTP client of DeepL and Umami.

use std::sync::Arc;
use std::time::Duration;

/// An HTTP client: rustls with the `ring` provider and the Mozilla roots
/// (`webpki-roots`, as sqlx does), no redirects, short timeouts.
/// `https_only` refuses plain http, including through a redirect.
pub(crate) fn http_client(
    https_only: bool,
    timeout: Duration,
    connect_timeout: Duration,
) -> Result<reqwest::Client, String> {
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| format!("TLS configuration: {e}"))?
    .with_root_certificates(roots)
    .with_no_client_auth();

    reqwest::Client::builder()
        .tls_backend_preconfigured(tls)
        .https_only(https_only)
        // A redirect could lead to plain http or another host: every URL
        // must point at the service itself.
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(connect_timeout)
        .timeout(timeout)
        .user_agent(concat!("pulsars-api/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("cannot build an HTTP client: {e}"))
}
