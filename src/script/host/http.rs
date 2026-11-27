use super::HostContext;
use crate::view::native_error;
use rhai::{Dynamic, EvalAltResult, Module, Shared};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use ureq::http::{HeaderName, HeaderValue, Uri};

pub(crate) type NativeResult = Result<Dynamic, Box<EvalAltResult>>;

const CACHE_TTL: Duration = Duration::from_secs(60);
const MAX_CACHE_ENTRIES: usize = 256;
const MAX_BODY_BYTES: u64 = 10 << 20;
const MAX_REDIRECTS: u32 = 5;

pub(crate) fn build_agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .max_redirects(0)
        .http_status_as_error(false)
        .build();
    ureq::Agent::new_with_config(config)
}

pub(crate) fn module(context: HostContext) -> Shared<Module> {
    let mut module = Module::new();
    let get_context = context.clone();
    let get_headers_context = context.clone();
    let json_context = context.clone();
    let json_headers_context = context;
    module.set_native_fn("get", move |url: &str| -> NativeResult {
        fetch_cached(&get_context, url, &Headers::new()).map(Dynamic::from)
    });
    module.set_native_fn("get", move |url: &str, headers: Dynamic| -> NativeResult {
        let headers = headers_of(&headers)?;
        fetch_cached(&get_headers_context, url, &headers).map(Dynamic::from)
    });
    module.set_native_fn("get_json", move |url: &str| -> NativeResult {
        fetch_json(&json_context, url, &Headers::new())
    });
    module.set_native_fn("get_json", move |url: &str, headers: Dynamic| -> NativeResult {
        let headers = headers_of(&headers)?;
        fetch_json(&json_headers_context, url, &headers)
    });
    module.into()
}

type Headers = BTreeMap<String, String>;

fn headers_of(value: &Dynamic) -> Result<Headers, Box<EvalAltResult>> {
    if value.is_unit() {
        return Ok(Headers::new());
    }
    rhai::serde::from_dynamic(value).map_err(|err| native_error(err.to_string()))
}

fn fetch_json(context: &HostContext, url: &str, headers: &Headers) -> NativeResult {
    let body = fetch_cached(context, url, headers)?;
    let value: serde_json::Value =
        serde_json::from_str(&body).map_err(|err| native_error(err.to_string()))?;
    to_dynamic(value)
}

fn fetch_cached(
    context: &HostContext,
    url: &str,
    headers: &Headers,
) -> Result<String, Box<EvalAltResult>> {
    let cacheable = headers.is_empty();
    if cacheable
        && let Ok(cache) = context.http_cache.lock()
        && let Some((fetched_at, body)) = cache.get(url)
        && fetched_at.elapsed() < CACHE_TTL
    {
        return Ok(body.clone());
    }
    let body = fetch_text(context, url, headers)?;
    if cacheable
        && let Ok(mut cache) = context.http_cache.lock()
    {
        cache.retain(|_, (fetched_at, _)| fetched_at.elapsed() < CACHE_TTL);
        if cache.len() >= MAX_CACHE_ENTRIES {
            cache.clear();
        }
        cache.insert(url.to_string(), (Instant::now(), body.clone()));
    }
    Ok(body)
}

fn fetch_text(
    context: &HostContext,
    url: &str,
    headers: &Headers,
) -> Result<String, Box<EvalAltResult>> {
    let bytes = fetch_bytes(context, url, headers)?;
    String::from_utf8(bytes).map_err(|err| native_error(err.to_string()))
}

fn fetch_bytes(
    context: &HostContext,
    url: &str,
    headers: &Headers,
) -> Result<Vec<u8>, Box<EvalAltResult>> {
    let mut current = url.to_string();
    for _ in 0..=MAX_REDIRECTS {
        let timeout = context.http_timeout();
        if timeout.is_zero() {
            return Err(native_error("script deadline exceeded".to_string()));
        }
        let current_uri = parse_http_uri(&current)
            .ok_or_else(|| native_error(format!("invalid HTTP URL: {current}")))?;
        if !uri_host_allowed(&context.hosts, &current_uri) {
            return Err(native_error(format!("host not allowed: {current}")));
        }

        let mut request = context.http_agent.get(&current);
        for (name, value) in headers {
            let name: HeaderName = name
                .parse()
                .map_err(|_| native_error(format!("invalid header name: {name}")))?;
            let value: HeaderValue = value
                .parse()
                .map_err(|_| native_error(format!("invalid header value for {name}")))?;
            request = request.header(name, value);
        }
        let mut response = request
            .config()
            .timeout_global(Some(timeout))
            .build()
            .call()
            .map_err(|err| native_error(err.to_string()))?;

        let status = response.status().as_u16();
        if (300..400).contains(&status) {
            let location = response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| native_error(format!("redirect without location from {current}")))?;
            current = resolve_redirect(&current_uri, location)?;
            continue;
        }
        if !(200..300).contains(&status) {
            return Err(native_error(format!("HTTP {status} from {current}")));
        }

        let bytes = response
            .body_mut()
            .with_config()
            .limit(MAX_BODY_BYTES)
            .read_to_vec()
            .map_err(|err| native_error(err.to_string()))?;
        return Ok(bytes);
    }
    Err(native_error(format!("too many redirects from {url}")))
}

fn parse_http_uri(raw: &str) -> Option<Uri> {
    let uri: Uri = raw.parse().ok()?;
    let scheme = uri.scheme_str()?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    uri.host()?;
    Some(uri)
}

fn uri_host_allowed(hosts: &[String], uri: &Uri) -> bool {
    let Some(host) = uri.host() else {
        return false;
    };
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    hosts
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(host) || allowed.eq_ignore_ascii_case(bare))
}

fn resolve_redirect(current: &Uri, location: &str) -> Result<String, Box<EvalAltResult>> {
    let scheme = current
        .scheme_str()
        .ok_or_else(|| native_error("redirect base missing scheme".to_string()))?;
    if location.starts_with("//") {
        return Ok(format!("{scheme}:{location}"));
    }
    if let Ok(parsed) = location.parse::<Uri>()
        && parsed.scheme().is_some()
    {
        return Ok(location.to_string());
    }
    let authority = current
        .authority()
        .map(ureq::http::uri::Authority::as_str)
        .ok_or_else(|| native_error("redirect base missing authority".to_string()))?;
    if location.starts_with('/') {
        return Ok(format!("{scheme}://{authority}{location}"));
    }
    if location.starts_with('?') {
        return Ok(format!(
            "{scheme}://{authority}{}{location}",
            current.path()
        ));
    }
    let parent = match current.path().rsplit_once('/') {
        Some((dir, _)) if !dir.is_empty() => dir,
        _ => "",
    };
    Ok(format!("{scheme}://{authority}{parent}/{location}"))
}

fn to_dynamic(value: serde_json::Value) -> NativeResult {
    rhai::serde::to_dynamic(value).map_err(|err| native_error(err.to_string()))
}
