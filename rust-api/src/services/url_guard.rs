// SSRF guard for server-side fetches of attacker-controlled URLs (e.g. podcast
// RSS enclosure URLs). Restricts the scheme to http/https and rejects any URL
// whose host resolves to a loopback, link-local, private, unique-local,
// unspecified or otherwise reserved address. IPv4-mapped IPv6 (::ffff:a.b.c.d)
// and NAT64 (64:ff9b::/96) forms are unwrapped before classification so they
// cannot be used to smuggle an internal IPv4 address past the check.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

/// Returns true if `ip` must never be the target of a server-side fetch.
fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_blocked_v4(v4),
        IpAddr::V6(v6) => {
            // Unwrap IPv4-mapped (::ffff:a.b.c.d) form and re-check as IPv4 so an
            // internal v4 target cannot be smuggled. Use to_ipv4_mapped() rather
            // than to_ipv4() — the latter also maps IPv4-compatible addresses
            // such as ::1 to 0.0.0.1, which would bypass the loopback check.
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_blocked_v4(v4);
            }
            // NAT64 well-known prefix 64:ff9b::/96 embeds an IPv4 address in the
            // low 32 bits; unwrap and re-check.
            let seg = v6.segments();
            if seg[0] == 0x0064 && seg[1] == 0xff9b && seg[2] == 0 && seg[3] == 0 && seg[4] == 0 && seg[5] == 0 {
                let o = v6.octets();
                let v4 = Ipv4Addr::new(o[12], o[13], o[14], o[15]);
                return is_blocked_v4(v4);
            }
            is_blocked_v6(v6)
        }
    }
}

fn is_blocked_v4(v4: Ipv4Addr) -> bool {
    v4.is_loopback()            // 127.0.0.0/8
        || v4.is_private()      // 10/8, 172.16/12, 192.168/16
        || v4.is_link_local()   // 169.254.0.0/16 (incl. cloud metadata)
        || v4.is_unspecified()  // 0.0.0.0
        || v4.is_broadcast()    // 255.255.255.255
        || v4.is_documentation()
        || v4.octets()[0] == 100 && (v4.octets()[1] & 0xc0) == 64 // 100.64.0.0/10 CGNAT
        || v4.octets()[0] >= 240 // 240.0.0.0/4 reserved
}

fn is_blocked_v6(v6: Ipv6Addr) -> bool {
    v6.is_loopback()            // ::1
        || v6.is_unspecified()  // ::
        || (v6.segments()[0] & 0xffc0) == 0xfe80 // fe80::/10 link-local
        || (v6.segments()[0] & 0xfe00) == 0xfc00 // fc00::/7 unique-local
        || (v6.segments()[0] & 0xff00) == 0xff00 // ff00::/8 multicast
}

/// Validate a URL before any server-side fetch.
///
/// Returns Ok(()) only if the scheme is http/https and *every* address the host
/// resolves to is a public, routable address. Resolution failure or any blocked
/// address yields Err with a short reason.
pub fn ensure_safe_public_url(raw_url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(raw_url).map_err(|e| format!("invalid URL: {}", e))?;

    match parsed.scheme() {
        "http" | "https" => {}
        other => return Err(format!("scheme '{}' not allowed (only http/https)", other)),
    }

    let host_raw = parsed
        .host_str()
        .ok_or_else(|| "URL has no host".to_string())?
        .to_string();

    // url::Url::host_str() returns IPv6 literals wrapped in brackets
    // (e.g. "[::1]"); strip them before attempting a literal-IP parse.
    let host = host_raw
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .map(|h| h.to_string())
        .unwrap_or(host_raw);

    // If the host is already a literal IP, classify it directly.
    if let Ok(ip) = host.parse::<IpAddr>() {
        return if is_blocked_ip(ip) {
            Err(format!("destination IP {} is private/reserved", ip))
        } else {
            Ok(())
        };
    }

    let port = parsed.port_or_known_default().unwrap_or(80);
    let addrs: Vec<IpAddr> = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| format!("could not resolve host '{}': {}", host, e))?
        .map(|sa| sa.ip())
        .collect();

    if addrs.is_empty() {
        return Err(format!("host '{}' resolved to no addresses", host));
    }
    for ip in &addrs {
        if is_blocked_ip(*ip) {
            return Err(format!(
                "host '{}' resolves to private/reserved address {}",
                host, ip
            ));
        }
    }
    Ok(())
}

/// Async wrapper: DNS resolution is blocking, so run the check off the async
/// executor.
pub async fn ensure_safe_public_url_async(raw_url: &str) -> Result<(), String> {
    let owned = raw_url.to_string();
    tokio::task::spawn_blocking(move || ensure_safe_public_url(&owned))
        .await
        .map_err(|e| format!("URL guard task failed: {}", e))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_loopback_and_metadata_and_private() {
        for u in [
            "http://127.0.0.1:8040/api/pinepods_check",
            "http://169.254.169.254/latest/meta-data/",
            "http://[::ffff:169.254.169.254]/",
            "http://[64:ff9b::a9fe:a9fe]/",
            "http://10.0.0.5/",
            "http://192.168.1.1/",
            "http://[::1]/",
            "gopher://127.0.0.1/",
            "file:///etc/passwd",
        ] {
            assert!(ensure_safe_public_url(u).is_err(), "should block {}", u);
        }
    }

    #[test]
    fn allows_public_literal() {
        assert!(ensure_safe_public_url("http://93.184.216.34/audio.mp3").is_ok());
    }
}
