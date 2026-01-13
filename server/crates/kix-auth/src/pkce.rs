//! PKCE (Proof Key for Code Exchange) support.
//!
//! Implements RFC 7636 PKCE with S256 challenge method.
//! This prevents authorization code interception attacks.

use sha2::{Digest, Sha256};

/// Supported PKCE challenge methods.
pub const CHALLENGE_METHOD_S256: &str = "S256";

/// Verifies a PKCE code verifier against a stored challenge.
///
/// # Arguments
/// * `verifier` - The code_verifier sent during token exchange
/// * `challenge` - The code_challenge stored during authorization
/// * `method` - The challenge method (must be "S256")
///
/// # Returns
/// `true` if the verifier matches the challenge, `false` otherwise.
pub fn verify_pkce(verifier: &str, challenge: &str, method: &str) -> bool {
    if method != CHALLENGE_METHOD_S256 {
        return false;
    }

    // Per RFC 7636:
    // code_challenge = BASE64URL(SHA256(ASCII(code_verifier)))
    let computed_challenge = compute_s256_challenge(verifier);
    computed_challenge == challenge
}

/// Computes the S256 challenge from a code verifier.
///
/// Per RFC 7636: code_challenge = BASE64URL(SHA256(ASCII(code_verifier)))
pub fn compute_s256_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();

    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD.encode(hash)
}

/// Validates that a code verifier meets RFC 7636 requirements.
///
/// Code verifier must be:
/// - Between 43 and 128 characters
/// - Contain only unreserved URI characters: [A-Z] [a-z] [0-9] - . _ ~
pub fn is_valid_verifier(verifier: &str) -> bool {
    let len = verifier.len();
    if !(43..=128).contains(&len) {
        return false;
    }

    verifier.chars().all(|c| {
        c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' || c == '~'
    })
}

/// Validates that a code challenge meets RFC 7636 requirements.
///
/// For S256 method, challenge should be:
/// - 43 characters (base64url-encoded SHA-256)
/// - Contain only URL-safe base64 characters: [A-Z] [a-z] [0-9] - _
pub fn is_valid_challenge(challenge: &str) -> bool {
    // S256 produces exactly 43 characters
    if challenge.len() != 43 {
        return false;
    }

    challenge
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_s256_challenge() {
        // Test vector from RFC 7636 Appendix B
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let expected_challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert_eq!(compute_s256_challenge(verifier), expected_challenge);
    }

    #[test]
    fn test_verify_pkce_success() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = compute_s256_challenge(verifier);
        assert!(verify_pkce(verifier, &challenge, CHALLENGE_METHOD_S256));
    }

    #[test]
    fn test_verify_pkce_wrong_verifier() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = compute_s256_challenge(verifier);
        assert!(!verify_pkce("wrong_verifier", &challenge, CHALLENGE_METHOD_S256));
    }

    #[test]
    fn test_verify_pkce_wrong_method() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = compute_s256_challenge(verifier);
        assert!(!verify_pkce(verifier, &challenge, "plain"));
    }

    #[test]
    fn test_is_valid_verifier() {
        // Valid verifiers
        assert!(is_valid_verifier(
            "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
        )); // 43 chars
        assert!(is_valid_verifier(&"a".repeat(43))); // Min length
        assert!(is_valid_verifier(&"a".repeat(128))); // Max length
        assert!(is_valid_verifier("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnop-._~")); // All valid chars

        // Invalid verifiers
        assert!(!is_valid_verifier(&"a".repeat(42))); // Too short
        assert!(!is_valid_verifier(&"a".repeat(129))); // Too long
        assert!(!is_valid_verifier(&format!("{}!", "a".repeat(42)))); // Invalid char
    }

    #[test]
    fn test_is_valid_challenge() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = compute_s256_challenge(verifier);
        assert!(is_valid_challenge(&challenge));

        // Invalid challenges
        assert!(!is_valid_challenge(&"a".repeat(42))); // Too short
        assert!(!is_valid_challenge(&"a".repeat(44))); // Too long
        assert!(!is_valid_challenge(&format!("{}!", "a".repeat(42)))); // Invalid char
    }
}
