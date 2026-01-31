use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref DOMAIN_PATTERN: Regex = Regex::new(
        r"^(?:(?:\*\.)?(?:[a-zA-Z0-9_*](?:[a-zA-Z0-9_*-]{0,61}[a-zA-Z0-9_*])?\.)+[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)$"
    ).unwrap();
}

#[inline]
pub fn is_valid_domain(domain: &str) -> bool {
    if domain.is_empty() || domain.len() > 253 {
        return false;
    }

    // Check for invalid patterns
    if domain.starts_with('*') && !domain.starts_with("*.") {
        return false;
    }

    if domain.ends_with('*') || domain == "*" {
        return false;
    }

    if domain.contains(".-") || domain.contains("-.") || domain.starts_with('.') || domain.ends_with('.') {
        return false;
    }

    DOMAIN_PATTERN.is_match(domain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_domains() {
        assert!(is_valid_domain("example.com"));
        assert!(is_valid_domain("sub.example.com"));
        assert!(is_valid_domain("*.example.com"));
        assert!(is_valid_domain("_service.example.com"));
        assert!(is_valid_domain("svc-*.domain.com"));
    }

    #[test]
    fn test_invalid_domains() {
        assert!(!is_valid_domain(""));
        assert!(!is_valid_domain("*"));
        assert!(!is_valid_domain("*."));
        assert!(!is_valid_domain("*abc.com"));
        assert!(!is_valid_domain("domain.*"));
        assert!(!is_valid_domain("-.example.com"));
    }
}
