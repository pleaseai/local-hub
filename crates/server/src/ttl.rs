use std::time::Duration;

/// Default TTL for cached responses.
const DEFAULT_TTL: Duration = Duration::from_secs(300); // 5 minutes

/// TTL rule: a path pattern and its associated TTL duration.
#[derive(Debug, Clone)]
pub struct TtlRule {
    pub pattern: String,
    pub ttl: Duration,
}

/// TTL configuration: ordered list of rules checked in order.
/// First matching pattern wins. Falls back to default TTL.
#[derive(Debug, Clone)]
pub struct TtlConfig {
    rules: Vec<TtlRule>,
    default_ttl: Duration,
}

impl TtlConfig {
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            rules: Vec::new(),
            default_ttl,
        }
    }

    /// Add a rule: paths starting with `pattern` get the specified TTL.
    pub fn add_rule(&mut self, pattern: impl Into<String>, ttl: Duration) {
        self.rules.push(TtlRule {
            pattern: pattern.into(),
            ttl,
        });
    }

    /// Resolve the TTL for a given path. First matching rule wins.
    pub fn resolve(&self, path: &str) -> Duration {
        for rule in &self.rules {
            if path.starts_with(&rule.pattern) {
                return rule.ttl;
            }
        }
        self.default_ttl
    }
}

impl Default for TtlConfig {
    fn default() -> Self {
        let mut config = Self::new(DEFAULT_TTL);
        // Frequently changing endpoints get shorter TTLs
        config.add_rule("/notifications", Duration::from_secs(60));
        // User-specific data changes often
        config.add_rule("/user", Duration::from_secs(120));
        // Search results can be cached longer
        config.add_rule("/search", Duration::from_secs(600));
        // Rate limit info should be fresh
        config.add_rule("/rate_limit", Duration::from_secs(30));
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_ttl() {
        let config = TtlConfig::default();
        assert_eq!(config.resolve("/repos/org/repo/issues"), DEFAULT_TTL);
    }

    #[test]
    fn test_notifications_short_ttl() {
        let config = TtlConfig::default();
        assert_eq!(config.resolve("/notifications"), Duration::from_secs(60));
    }

    #[test]
    fn test_notifications_subpath() {
        let config = TtlConfig::default();
        assert_eq!(
            config.resolve("/notifications/threads/123"),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn test_search_longer_ttl() {
        let config = TtlConfig::default();
        assert_eq!(
            config.resolve("/search/repositories?q=rust"),
            Duration::from_secs(600)
        );
    }

    #[test]
    fn test_rate_limit_very_short() {
        let config = TtlConfig::default();
        assert_eq!(config.resolve("/rate_limit"), Duration::from_secs(30));
    }

    #[test]
    fn test_custom_rule() {
        let mut config = TtlConfig::new(Duration::from_secs(300));
        config.add_rule("/orgs", Duration::from_secs(900));
        assert_eq!(
            config.resolve("/orgs/myorg/repos"),
            Duration::from_secs(900)
        );
    }

    #[test]
    fn test_first_match_wins() {
        let mut config = TtlConfig::new(Duration::from_secs(300));
        config.add_rule("/repos", Duration::from_secs(100));
        config.add_rule("/repos/org", Duration::from_secs(200));
        // First rule matches, so 100s
        assert_eq!(config.resolve("/repos/org/repo"), Duration::from_secs(100));
    }

    #[test]
    fn test_custom_default_ttl() {
        let config = TtlConfig::new(Duration::from_secs(60));
        assert_eq!(
            config.resolve("/some/unknown/path"),
            Duration::from_secs(60)
        );
    }
}
