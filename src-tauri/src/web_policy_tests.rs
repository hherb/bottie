//! User-configurable native Web network policy tests.

use url::Url;

use crate::web_policy::{MAX_WEB_POLICY_DOMAINS, WebNetworkPolicy};

#[test]
fn defaults_to_https_only_without_narrowing_public_domains() {
    let policy = WebNetworkPolicy::default();

    assert!(policy.https_only);
    assert!(policy.allowed_domains.is_empty());
    assert!(policy.blocked_domains.is_empty());
    assert!(policy.allows_url(&Url::parse("https://www.iana.org/domains").unwrap()));
    assert!(!policy.allows_url(&Url::parse("http://www.iana.org/domains").unwrap()));
}

#[test]
fn normalizes_bounded_domains_and_applies_blocked_precedence() {
    let policy = WebNetworkPolicy {
        https_only: false,
        allowed_domains: vec![" Rust-Lang.ORG. ".into(), "docs.rs".into()],
        blocked_domains: vec!["Ads.Rust-Lang.org".into()],
    }
    .normalized()
    .unwrap();

    assert_eq!(policy.allowed_domains, ["rust-lang.org", "docs.rs"]);
    assert_eq!(policy.blocked_domains, ["ads.rust-lang.org"]);
    assert!(policy.allows_url(&Url::parse("http://docs.rust-lang.org/book").unwrap()));
    assert!(!policy.allows_url(&Url::parse("https://ads.rust-lang.org/tracker").unwrap()));
    assert!(!policy.allows_url(&Url::parse("https://www.iana.org/domains").unwrap()));
}

#[test]
fn rejects_unsafe_conflicting_or_unbounded_domain_settings() {
    for domain in [
        "https://rust-lang.org/book",
        "*.rust-lang.org",
        "127.0.0.1",
        "localhost",
        "device.local",
    ] {
        let policy = WebNetworkPolicy {
            allowed_domains: vec![domain.into()],
            ..WebNetworkPolicy::default()
        };
        assert!(
            policy.normalized().is_err(),
            "unsafe domain passed: {domain}"
        );
    }

    let conflicting = WebNetworkPolicy {
        allowed_domains: vec!["rust-lang.org".into()],
        blocked_domains: vec!["RUST-LANG.ORG".into()],
        ..WebNetworkPolicy::default()
    };
    assert!(conflicting.normalized().is_err());

    let unbounded = WebNetworkPolicy {
        blocked_domains: (0..=MAX_WEB_POLICY_DOMAINS)
            .map(|index| format!("domain-{index}.org"))
            .collect(),
        ..WebNetworkPolicy::default()
    };
    assert!(unbounded.normalized().is_err());
}
