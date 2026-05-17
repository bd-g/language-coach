use axum::http::HeaderMap;
use subtle::ConstantTimeEq;

/// Extracts the debug auth token from standard headers.
pub(crate) fn extract_debug_token(headers: &HeaderMap) -> Option<String> {
    if let Some(auth) = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
    {
        return Some(auth.to_owned());
    }
    let protocols = headers
        .get("sec-websocket-protocol")
        .and_then(|value| value.to_str().ok())?;
    protocols
        .split(',')
        .map(str::trim)
        .find_map(|part| part.strip_prefix("bearer."))
        .map(ToOwned::to_owned)
}

/// Compares tokens without timing side channels.
pub(crate) fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    left.ct_eq(right).into()
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderMap;

    use super::extract_debug_token;

    #[test]
    fn parses_bearer_token_from_authorization_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            axum::http::HeaderValue::from_static("Bearer abc123"),
        );
        let token = extract_debug_token(&headers);
        assert_eq!(token.as_deref(), Some("abc123"));
    }

    #[test]
    fn parses_bearer_token_from_subprotocol() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "sec-websocket-protocol",
            axum::http::HeaderValue::from_static("language-coach-debug, bearer.secret"),
        );
        let token = extract_debug_token(&headers);
        assert_eq!(token.as_deref(), Some("secret"));
    }
}
