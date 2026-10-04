//! Per-IP rate limiting, as `docs/api.md` describes: a 60-second sliding window
//! per client address (an IPv6 client's /64) and endpoint group, kept in Redis
//! so every API replica shares it, under a keyed hash of the address. Over the
//! limit, a request gets 429 with `Retry-After`. Every limited response
//! carries `X-RateLimit-Limit` and `X-RateLimit-Remaining`.

use std::net::{IpAddr, SocketAddr};
use std::sync::LazyLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use hmac::{Hmac, Mac};
use purestat_config::RateLimitSettings;
use sha2::Sha256;

use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

const WINDOW_MS: u64 = 60_000;

/// How long a request waits on the limiter. A stalled Redis must not stall
/// every request behind it, so past this the request is let through.
const REDIS_TIMEOUT: Duration = Duration::from_millis(500);

/// Atomically: drop entries older than the window, and admit the request if
/// fewer than `limit` remain. A refused request is not recorded, so a client
/// that stops is admitted again as soon as its oldest request ages out.
/// Returns {allowed, remaining, retry_after_ms}.
static SLIDING_WINDOW: LazyLock<redis::Script> = LazyLock::new(|| {
    redis::Script::new(
        r#"
local key = KEYS[1]
local now = tonumber(ARGV[1])
local window = tonumber(ARGV[2])
local limit = tonumber(ARGV[3])
redis.call('ZREMRANGEBYSCORE', key, 0, now - window)
local count = redis.call('ZCARD', key)
if count < limit then
  redis.call('ZADD', key, now, ARGV[4])
  redis.call('PEXPIRE', key, window)
  return {1, limit - count - 1, 0}
end
local oldest = redis.call('ZRANGE', key, 0, 0, 'WITHSCORES')
if #oldest == 0 then
  return {0, 0, window}
end
return {0, 0, window - (now - tonumber(oldest[2]))}
"#,
    )
});

/// The group a path belongs to, and its limit per minute. `None` means not
/// limited: the health check, and Stripe's webhook (signed, and sent from
/// Stripe's own addresses).
pub fn classify(path: &str, limits: &RateLimitSettings) -> Option<(&'static str, u32)> {
    if !path.starts_with("/api/") || path == "/api/health" || path == "/api/stripe/webhook" {
        return None;
    }
    if path.starts_with("/api/auth/") || path.starts_with("/api/oauth/") {
        return Some(("auth", limits.auth_rpm));
    }
    if path == "/api/event" {
        return Some(("tracker", limits.tracker_rpm));
    }
    if path.ends_with("/stats") || path.ends_with("/export") {
        return Some(("stats", limits.stats_rpm));
    }
    Some(("api", limits.api_rpm))
}

/// The client's address, as the proxies in front of the API saw it.
///
/// A front proxy APPENDS the address it saw to `X-Forwarded-For`, so any
/// entries a client sends itself sit on the left and cannot be trusted. The
/// first address from the right that is not a proxy hop (private, loopback,
/// link-local) is the client. Without one, `X-Real-IP` (when it is a public
/// address), then the right-most entry, then the TCP peer.
pub fn client_ip(headers: &HeaderMap, peer: Option<IpAddr>) -> Option<IpAddr> {
    let forwarded: Vec<IpAddr> = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .filter_map(|s| s.trim().parse::<IpAddr>().ok())
        .collect();
    if let Some(ip) = forwarded.iter().rev().find(|ip| !is_proxy_hop(ip)) {
        return Some(*ip);
    }
    let real_ip = headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<IpAddr>().ok());
    if let Some(ip) = real_ip.filter(|ip| !is_proxy_hop(ip)) {
        return Some(ip);
    }
    // A client on the proxies' own network, or no proxy at all.
    forwarded.last().copied().or(peer)
}

/// What one counter covers: an IPv4 address, or an IPv6 client's /64. A
/// single IPv6 host usually holds a whole /64, so counting per address would
/// let it step around every limit by changing addresses.
pub fn bucket(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => {
                let s = v6.segments();
                format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
            }
        },
    }
}

/// The counter's name in Redis: a keyed hash of the bucket, so Redis never
/// holds a client's address (docs/data-model.md: the raw IP is never stored).
/// The key is the server secret every replica shares, so they share counters.
pub fn bucket_tag(secret: &str, bucket: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC takes a key of any length");
    mac.update(b"purestat rate limit\0");
    mac.update(bucket.as_bytes());
    hex::encode(&mac.finalize().into_bytes()[..16])
}

fn is_proxy_hop(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || (a == 100 && (b & 0xC0) == 64) // 100.64.0.0/10, carrier-grade NAT
        }
        IpAddr::V6(v6) => {
            let first = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || (first & 0xfe00) == 0xfc00 // unique local
                || (first & 0xffc0) == 0xfe80 // link-local
                || v6.to_ipv4_mapped().is_some_and(|v4| is_proxy_hop(&IpAddr::V4(v4)))
        }
    }
}

struct Decision {
    allowed: bool,
    remaining: u32,
    retry_after_ms: u64,
}

async fn check(
    redis: &mut redis::aio::ConnectionManager,
    group: &str,
    tag: &str,
    limit: u32,
) -> redis::RedisResult<Decision> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default();
    let member = format!("{now}-{}", nanoid::nanoid!(8));
    let (allowed, remaining, retry_after_ms): (i64, i64, i64) = SLIDING_WINDOW
        .key(format!("purestat:rl:{group}:{tag}"))
        .arg(now)
        .arg(WINDOW_MS)
        .arg(limit)
        .arg(member)
        .invoke_async(redis)
        .await?;
    Ok(Decision {
        allowed: allowed == 1,
        remaining: remaining.max(0) as u32,
        retry_after_ms: retry_after_ms.max(0) as u64,
    })
}

fn set_limit_headers(headers: &mut HeaderMap, limit: u32, remaining: u32) {
    headers.insert("x-ratelimit-limit", HeaderValue::from(limit));
    headers.insert("x-ratelimit-remaining", HeaderValue::from(remaining));
}

/// The middleware. A Redis failure or timeout lets the request through: an
/// outage of the limiter must not take logins and event ingest down with it.
pub async fn limit(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let settings = &state.settings.rate_limit;
    if !settings.enabled {
        return next.run(req).await;
    }
    let Some((group, limit)) = classify(req.uri().path(), settings) else {
        return next.run(req).await;
    };
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip());
    let bucket = client_ip(req.headers(), peer).map_or_else(|| "unknown".to_string(), bucket);
    let tag = bucket_tag(&state.settings.jwt.secret, &bucket);

    let mut redis = state.redis.clone();
    let checked = tokio::time::timeout(REDIS_TIMEOUT, check(&mut redis, group, &tag, limit)).await;
    match checked {
        Ok(Ok(decision)) if decision.allowed => {
            let mut response = next.run(req).await;
            set_limit_headers(response.headers_mut(), limit, decision.remaining);
            response
        }
        Ok(Ok(decision)) => {
            let retry = decision.retry_after_ms.div_ceil(1000).max(1);
            tracing::info!(group = group, path = %req.uri().path(), retry_after_secs = retry, "Rate limited");
            let body = serde_json::json!({
                "error": "rate_limited",
                "message": format!("Too many requests. Retry in {retry} s."),
            });
            let mut response = (StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response();
            response
                .headers_mut()
                .insert("retry-after", HeaderValue::from(retry));
            set_limit_headers(response.headers_mut(), limit, 0);
            response
        }
        Ok(Err(e)) => {
            tracing::warn!(error = %e, group = group, "Rate limiter unavailable; the request is allowed");
            next.run(req).await
        }
        Err(_) => {
            tracing::warn!(
                group = group,
                "Rate limiter timed out; the request is allowed"
            );
            next.run(req).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> RateLimitSettings {
        RateLimitSettings::default()
    }

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.append(*k, HeaderValue::from_str(v).unwrap());
        }
        h
    }

    fn ip(s: &str) -> Option<IpAddr> {
        Some(s.parse().unwrap())
    }

    #[test]
    fn paths_fall_into_the_documented_groups() {
        let l = limits();
        for (path, group) in [
            ("/api/auth/login", Some("auth")),
            ("/api/auth/resend-activation", Some("auth")),
            ("/api/oauth/google/callback", Some("auth")),
            ("/api/event", Some("tracker")),
            ("/api/org/a/site/b/stats", Some("stats")),
            ("/api/org/a/site/b/export", Some("stats")),
            ("/api/org/a/analytics/stats", Some("stats")),
            ("/api/org/a/analytics/realtime", Some("api")),
            ("/api/me", Some("api")),
            ("/api/health", None),
            ("/api/stripe/webhook", None),
            ("/assets/x.js", None),
        ] {
            assert_eq!(classify(path, &l).map(|(g, _)| g), group, "{path}");
        }
    }

    #[test]
    fn the_client_is_the_rightmost_public_forwarded_address() {
        // The front proxy appended the address it saw; the client wrote the rest.
        let h = headers(&[("x-forwarded-for", "1.1.1.1, 9.9.9.9, 203.0.113.7")]);
        assert_eq!(client_ip(&h, None), ip("203.0.113.7"));
    }

    #[test]
    fn proxy_hops_appended_after_the_client_are_skipped() {
        let h = headers(&[(
            "x-forwarded-for",
            "8.8.8.8, 203.0.113.7, 10.10.20.11, 127.0.0.1",
        )]);
        assert_eq!(client_ip(&h, None), ip("203.0.113.7"));
    }

    #[test]
    fn without_forwarding_headers_the_peer_is_the_client() {
        let peer = ip("198.51.100.4");
        assert_eq!(client_ip(&HeaderMap::new(), peer), peer);
        assert_eq!(client_ip(&HeaderMap::new(), None), None);
    }

    #[test]
    fn a_public_x_real_ip_counts_only_when_forwarded_for_has_none() {
        let h = headers(&[
            ("x-forwarded-for", "10.0.0.5"),
            ("x-real-ip", "203.0.113.9"),
        ]);
        assert_eq!(client_ip(&h, None), ip("203.0.113.9"));
        let h = headers(&[
            ("x-forwarded-for", "203.0.113.1"),
            ("x-real-ip", "203.0.113.9"),
        ]);
        assert_eq!(client_ip(&h, None), ip("203.0.113.1"));
    }

    #[test]
    fn a_client_on_the_proxies_own_network_is_its_own_address() {
        let h = headers(&[("x-forwarded-for", "192.168.1.20")]);
        assert_eq!(client_ip(&h, ip("10.0.0.1")), ip("192.168.1.20"));
    }

    #[test]
    fn garbage_entries_are_ignored() {
        let h = headers(&[("x-forwarded-for", "unknown, not-an-ip, 203.0.113.7")]);
        assert_eq!(client_ip(&h, None), ip("203.0.113.7"));
    }

    #[test]
    fn ipv6_proxy_hops_are_skipped_too() {
        let h = headers(&[("x-forwarded-for", "2001:db8::1, fd00::2, ::1")]);
        assert_eq!(client_ip(&h, None), ip("2001:db8::1"));
    }

    #[test]
    fn an_ipv6_client_is_counted_by_its_slash_64() {
        let b = |s: &str| bucket(s.parse().unwrap());
        assert_eq!(b("2001:db8:1:2::7"), b("2001:db8:1:2:ffff:ffff:ffff:ffff"));
        assert_eq!(b("2001:db8:1:2::7"), "2001:db8:1:2::/64");
        assert_ne!(b("2001:db8:1:2::7"), b("2001:db8:1:3::7"));
    }

    #[test]
    fn an_ipv4_client_is_counted_by_its_address_however_it_is_written() {
        let b = |s: &str| bucket(s.parse().unwrap());
        assert_eq!(b("203.0.113.7"), "203.0.113.7");
        assert_eq!(b("::ffff:203.0.113.7"), "203.0.113.7");
        assert_ne!(b("203.0.113.7"), b("203.0.113.8"));
    }

    #[test]
    fn the_redis_key_does_not_carry_the_address() {
        let tag = bucket_tag("secret", "203.0.113.7");
        assert_eq!(tag.len(), 32);
        assert!(!tag.contains("203"), "{tag}");
        assert_eq!(tag, bucket_tag("secret", "203.0.113.7"), "stable");
        assert_ne!(tag, bucket_tag("secret", "203.0.113.8"));
        assert_ne!(tag, bucket_tag("other secret", "203.0.113.7"), "keyed");
    }
}
