//! SSRF (Server-Side Request Forgery) Protection
//!
//! Validates URLs to prevent SSRF attacks by blocking:
//! - Private IP ranges (RFC 1918, RFC 4193)
//! - Localhost and loopback addresses
//! - Link-local addresses
//! - Cloud metadata endpoints

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use thiserror::Error;
use url::Url;

#[derive(Error, Debug)]
pub enum SsrfError {
    #[error("URL resolves to private IP address: {0}")]
    PrivateIp(IpAddr),

    #[error("URL resolves to localhost: {0}")]
    Localhost(IpAddr),

    #[error("URL resolves to link-local address: {0}")]
    LinkLocal(IpAddr),

    #[error("URL targets cloud metadata endpoint: {0}")]
    CloudMetadata(String),

    #[error("URL uses blocked scheme: {0}")]
    BlockedScheme(String),

    #[error("URL hostname could not be resolved: {0}")]
    UnresolvableHost(String),

    #[error("Redirect to blocked destination: {0}")]
    BlockedRedirect(String),
}

/// SSRF validator that checks URLs for potential security risks
#[derive(Debug, Clone)]
pub struct SsrfValidator {
    /// Allow localhost connections (useful for development)
    allow_localhost: bool,
    /// Allow private IP ranges
    allow_private: bool,
}

impl Default for SsrfValidator {
    fn default() -> Self {
        Self {
            allow_localhost: false,
            allow_private: false,
        }
    }
}

impl SsrfValidator {
    /// Create a new SSRF validator with default settings (strict mode)
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a development-mode validator that allows localhost
    pub fn development() -> Self {
        Self {
            allow_localhost: true,
            allow_private: true,
        }
    }

    /// Set whether to allow localhost connections
    pub fn with_localhost(mut self, allow: bool) -> Self {
        self.allow_localhost = allow;
        self
    }

    /// Set whether to allow private IP ranges
    pub fn with_private(mut self, allow: bool) -> Self {
        self.allow_private = allow;
        self
    }

    /// Validate a URL for SSRF risks
    pub fn validate(&self, url: &Url) -> Result<(), SsrfError> {
        // Check scheme
        self.validate_scheme(url)?;

        // Check for cloud metadata endpoints
        self.validate_cloud_metadata(url)?;

        // Resolve and check IP address
        self.validate_resolved_ip(url)?;

        Ok(())
    }

    /// Validate URL scheme
    fn validate_scheme(&self, url: &Url) -> Result<(), SsrfError> {
        match url.scheme() {
            "http" | "https" => Ok(()),
            scheme => Err(SsrfError::BlockedScheme(scheme.to_string())),
        }
    }

    /// Check for cloud metadata endpoints
    fn validate_cloud_metadata(&self, url: &Url) -> Result<(), SsrfError> {
        let host = url.host_str().unwrap_or("");

        // AWS metadata
        if host == "169.254.169.254" {
            return Err(SsrfError::CloudMetadata("AWS metadata endpoint".to_string()));
        }

        // GCP metadata
        if host == "metadata.google.internal" || host == "169.254.169.254" {
            return Err(SsrfError::CloudMetadata("GCP metadata endpoint".to_string()));
        }

        // Azure metadata
        if host == "169.254.169.254" {
            return Err(SsrfError::CloudMetadata(
                "Azure metadata endpoint".to_string(),
            ));
        }

        // DigitalOcean metadata
        if host == "169.254.169.254" {
            return Err(SsrfError::CloudMetadata(
                "DigitalOcean metadata endpoint".to_string(),
            ));
        }

        Ok(())
    }

    /// Resolve hostname and validate IP address
    fn validate_resolved_ip(&self, url: &Url) -> Result<(), SsrfError> {
        let host = url.host_str().unwrap_or("");
        let port = url.port().unwrap_or(match url.scheme() {
            "https" => 443,
            _ => 80,
        });

        // Try to resolve the hostname
        let socket_addr = format!("{}:{}", host, port);
        let addrs = socket_addr
            .to_socket_addrs()
            .map_err(|_| SsrfError::UnresolvableHost(host.to_string()))?;

        // Check all resolved addresses
        for addr in addrs {
            let ip = addr.ip();
            self.validate_ip(ip)?;
        }

        Ok(())
    }

    /// Validate an IP address
    fn validate_ip(&self, ip: IpAddr) -> Result<(), SsrfError> {
        match ip {
            IpAddr::V4(ipv4) => self.validate_ipv4(ipv4),
            IpAddr::V6(ipv6) => self.validate_ipv6(ipv6),
        }
    }

    /// Validate an IPv4 address
    fn validate_ipv4(&self, ip: Ipv4Addr) -> Result<(), SsrfError> {
        // Localhost (127.0.0.0/8)
        if ip.is_loopback() {
            if !self.allow_localhost {
                return Err(SsrfError::Localhost(IpAddr::V4(ip)));
            }
        }

        // Private ranges (RFC 1918)
        if ip.is_private() && !self.allow_private {
            return Err(SsrfError::PrivateIp(IpAddr::V4(ip)));
        }

        // Link-local (169.254.0.0/16)
        if ip.is_link_local() {
            return Err(SsrfError::LinkLocal(IpAddr::V4(ip)));
        }

        // Broadcast
        if ip.is_broadcast() {
            return Err(SsrfError::PrivateIp(IpAddr::V4(ip)));
        }

        // Documentation ranges (192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24)
        if ip.is_documentation() {
            return Err(SsrfError::PrivateIp(IpAddr::V4(ip)));
        }

        // Unspecified (0.0.0.0)
        if ip.is_unspecified() {
            return Err(SsrfError::PrivateIp(IpAddr::V4(ip)));
        }

        Ok(())
    }

    /// Validate an IPv6 address
    fn validate_ipv6(&self, ip: Ipv6Addr) -> Result<(), SsrfError> {
        // Localhost (::1)
        if ip.is_loopback() {
            if !self.allow_localhost {
                return Err(SsrfError::Localhost(IpAddr::V6(ip)));
            }
        }

        // Unspecified (::)
        if ip.is_unspecified() {
            return Err(SsrfError::PrivateIp(IpAddr::V6(ip)));
        }

        // IPv4-mapped addresses - check the IPv4 portion
        if let Some(ipv4) = ip.to_ipv4_mapped() {
            return self.validate_ipv4(ipv4);
        }

        // Unique local addresses (fc00::/7) - equivalent to private IPv4
        let segments = ip.segments();
        if (segments[0] & 0xfe00) == 0xfc00 && !self.allow_private {
            return Err(SsrfError::PrivateIp(IpAddr::V6(ip)));
        }

        // Link-local (fe80::/10)
        if (segments[0] & 0xffc0) == 0xfe80 {
            return Err(SsrfError::LinkLocal(IpAddr::V6(ip)));
        }

        Ok(())
    }

    /// Validate a redirect target URL
    pub fn validate_redirect(&self, original: &Url, redirect: &Url) -> Result<(), SsrfError> {
        // Validate the redirect URL itself
        self.validate(redirect)?;

        // Check for protocol downgrade
        if original.scheme() == "https" && redirect.scheme() == "http" {
            // Allow protocol downgrade but log it
            tracing::warn!(
                "Protocol downgrade detected: {} -> {}",
                original,
                redirect
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_public_url_allowed() {
        let validator = SsrfValidator::new();
        let url = Url::parse("https://example.com").unwrap();
        assert!(validator.validate(&url).is_ok());
    }

    #[test]
    fn test_localhost_blocked_by_default() {
        let validator = SsrfValidator::new();
        let url = Url::parse("http://127.0.0.1").unwrap();
        assert!(matches!(
            validator.validate(&url),
            Err(SsrfError::Localhost(_))
        ));
    }

    #[test]
    fn test_localhost_allowed_in_dev_mode() {
        let validator = SsrfValidator::development();
        let url = Url::parse("http://127.0.0.1:3000").unwrap();
        assert!(validator.validate(&url).is_ok());
    }

    #[test]
    fn test_private_ip_blocked() {
        let validator = SsrfValidator::new();
        let url = Url::parse("http://192.168.1.1").unwrap();
        assert!(matches!(
            validator.validate(&url),
            Err(SsrfError::PrivateIp(_))
        ));
    }

    #[test]
    fn test_cloud_metadata_blocked() {
        let validator = SsrfValidator::new();
        let url = Url::parse("http://169.254.169.254/latest/meta-data/").unwrap();
        assert!(matches!(
            validator.validate(&url),
            Err(SsrfError::CloudMetadata(_) | SsrfError::LinkLocal(_))
        ));
    }

    #[test]
    fn test_blocked_scheme() {
        let validator = SsrfValidator::new();
        let url = Url::parse("file:///etc/passwd").unwrap();
        assert!(matches!(
            validator.validate(&url),
            Err(SsrfError::BlockedScheme(_))
        ));
    }
}
