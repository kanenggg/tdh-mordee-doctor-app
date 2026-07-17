use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use std::convert::Infallible;
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Language {
    Thai,
    English,
}

impl Language {
    pub fn lang_code(&self) -> &str {
        match self {
            Language::Thai => "th-TH",
            Language::English => "en-US",
        }
    }

    pub fn json_key(&self) -> &str {
        match self {
            Language::Thai => "th",
            Language::English => "en",
        }
    }
}

impl Default for Language {
    fn default() -> Self {
        Language::Thai
    }
}

pub struct LanguagePreference(pub Language);

impl<S> FromRequestParts<S> for LanguagePreference
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let language = parts
            .headers
            .get("accept-language")
            .and_then(|v| v.to_str().ok())
            .map(|v| {
                let tag = v.split(',').next().unwrap_or("").trim();
                match tag {
                    "th-TH" | "th" => Language::Thai,
                    "en-US" | "en" => Language::English,
                    _ => Language::default(),
                }
            })
            .unwrap_or_default();

        async move { Ok(LanguagePreference(language)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;

    async fn extract_language(accept_language: Option<&str>) -> Language {
        let mut builder = Request::builder().method("GET").uri("/test");
        if let Some(val) = accept_language {
            builder = builder.header("accept-language", val);
        }
        let request = builder.body(()).unwrap();
        let (mut parts, _) = request.into_parts();
        let LanguagePreference(lang) = LanguagePreference::from_request_parts(&mut parts, &())
            .await
            .unwrap();
        lang
    }

    #[tokio::test]
    async fn th_th_header_returns_thai() {
        assert_eq!(extract_language(Some("th-TH")).await, Language::Thai);
    }

    #[tokio::test]
    async fn th_header_returns_thai() {
        assert_eq!(extract_language(Some("th")).await, Language::Thai);
    }

    #[tokio::test]
    async fn en_us_header_returns_english() {
        assert_eq!(extract_language(Some("en-US")).await, Language::English);
    }

    #[tokio::test]
    async fn en_header_returns_english() {
        assert_eq!(extract_language(Some("en")).await, Language::English);
    }

    #[tokio::test]
    async fn missing_header_defaults_to_thai() {
        assert_eq!(extract_language(None).await, Language::Thai);
    }

    #[tokio::test]
    async fn unrecognized_header_defaults_to_thai() {
        assert_eq!(extract_language(Some("fr-FR")).await, Language::Thai);
    }
}
