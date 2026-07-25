use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AclMatchRule {
    #[serde(default = "default_regex_any")]
    pub method: String,
    #[serde(default = "default_regex_any")]
    pub domain: String,
    #[serde(default = "default_regex_any")]
    pub path: String,
}

impl Default for AclMatchRule {
    fn default() -> Self {
        Self {
            method: default_regex_any(),
            domain: default_regex_any(),
            path: default_regex_any(),
        }
    }
}

fn default_regex_any() -> String {
    ".*".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AclRule {
    #[serde(default)]
    pub allow: Vec<AclMatchRule>,
    #[serde(default)]
    pub deny: Vec<AclMatchRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AclConfig {
    #[serde(default)]
    pub cookie_domains: HashMap<String, usize>,
    #[serde(default)]
    pub acl_rules: HashMap<String, AclRule>,
}

#[derive(Debug, Clone)]
pub struct CompiledMatchRule {
    pub method_re: Regex,
    pub domain_re: Regex,
    pub path_re: Regex,
}

#[derive(Debug, Clone)]
pub struct CompiledAclRule {
    pub allow: Vec<CompiledMatchRule>,
    pub deny: Vec<CompiledMatchRule>,
}

#[derive(Debug, Clone, Default)]
pub struct CompiledAclConfig {
    pub cookie_domains: Vec<(Regex, usize, String)>,
    pub acl_rules: HashMap<String, CompiledAclRule>,
}

impl CompiledAclConfig {
    /// Evaluate an incoming request (method, domain, path) against a rule name.
    /// Returns true if allowed, false if denied.
    /// Deny rules have higher priority than allow rules.
    pub fn evaluate(&self, rule_name: &str, method: &str, domain: &str, path: &str) -> bool {
        let rule = match self.acl_rules.get(rule_name) {
            Some(r) => r,
            None => return false, // Unknown rule defaults to deny
        };

        // 1. Check DENY rules first (highest priority)
        for deny_rule in &rule.deny {
            if deny_rule.method_re.is_match(method)
                && deny_rule.domain_re.is_match(domain)
                && deny_rule.path_re.is_match(path)
            {
                return false;
            }
        }

        // 2. Check ALLOW rules second
        for allow_rule in &rule.allow {
            if allow_rule.method_re.is_match(method)
                && allow_rule.domain_re.is_match(domain)
                && allow_rule.path_re.is_match(path)
            {
                return true;
            }
        }

        // 3. Default to DENY if no allow rule matches
        false
    }

    /// Resolve cookie domain scope for a given request host.
    /// Returns Some("target_domain") if matched, or None if no match (exact host scope).
    pub fn resolve_cookie_domain(&self, host: &str) -> Option<String> {
        // Strip optional port from host if present
        let clean_host = host.split(':').next().unwrap_or(host);

        for (re, level, pattern_str) in &self.cookie_domains {
            if re.is_match(clean_host) {
                return Some(extract_parent_domain(clean_host, pattern_str, *level));
            }
        }
        None
    }
}

/// Helper function to calculate domain level count from a pattern string
fn get_domain_level_count(pattern_str: &str) -> usize {
    let clean_pattern = pattern_str
        .trim_start_matches('^')
        .trim_start_matches(".*")
        .trim_start_matches('*')
        .trim_start_matches('\\')
        .trim_start_matches('.')
        .trim_end_matches('$');
    let parts: Vec<&str> = clean_pattern.split('.').filter(|s| !s.is_empty()).collect();
    parts.len()
}

/// Helper function to extract parent domain based on level offset
fn extract_parent_domain(host: &str, pattern_str: &str, levels: usize) -> String {
    let base_count = get_domain_level_count(pattern_str);
    let host_parts: Vec<&str> = host.split('.').filter(|s| !s.is_empty()).collect();

    let count = if base_count > 0 {
        base_count
    } else {
        host_parts.len()
    };

    let target_parts_count = count.saturating_sub(levels).max(1);

    if host_parts.len() >= target_parts_count {
        let start_idx = host_parts.len() - target_parts_count;
        host_parts[start_idx..].join(".")
    } else {
        host.to_string()
    }
}

/// Create default fallback AclConfig containing `allow_all` and `deny_all`
pub fn default_acl_config() -> AclConfig {
    let mut acl_rules = HashMap::new();

    acl_rules.insert(
        "allow_all".to_string(),
        AclRule {
            allow: vec![AclMatchRule {
                method: ".*".to_string(),
                domain: ".*".to_string(),
                path: ".*".to_string(),
            }],
            deny: vec![],
        },
    );

    acl_rules.insert(
        "deny_all".to_string(),
        AclRule {
            allow: vec![],
            deny: vec![AclMatchRule {
                method: ".*".to_string(),
                domain: ".*".to_string(),
                path: ".*".to_string(),
            }],
        },
    );

    AclConfig {
        cookie_domains: HashMap::new(),
        acl_rules,
    }
}

/// Parse and validate YAML string into AclConfig and CompiledAclConfig.
/// Enforces default rules (`allow_all` and `deny_all`) if missing.
pub fn parse_and_validate_yaml(yaml_str: &str) -> Result<(AclConfig, CompiledAclConfig), String> {
    let mut config: AclConfig = if yaml_str.trim().is_empty() {
        default_acl_config()
    } else {
        serde_yaml::from_str(yaml_str).map_err(|e| format!("YAML parse error: {}", e))?
    };

    // Ensure default rules exist
    if !config.acl_rules.contains_key("allow_all") {
        config.acl_rules.insert(
            "allow_all".to_string(),
            AclRule {
                allow: vec![AclMatchRule {
                    method: ".*".to_string(),
                    domain: ".*".to_string(),
                    path: ".*".to_string(),
                }],
                deny: vec![],
            },
        );
    }

    if !config.acl_rules.contains_key("deny_all") {
        config.acl_rules.insert(
            "deny_all".to_string(),
            AclRule {
                allow: vec![],
                deny: vec![AclMatchRule {
                    method: ".*".to_string(),
                    domain: ".*".to_string(),
                    path: ".*".to_string(),
                }],
            },
        );
    }

    // Validate and compile cookie_domains mapping
    let mut compiled_cookie_domains = Vec::new();
    for (pattern, &level) in &config.cookie_domains {
        if level < 1 {
            return Err(format!(
                "Invalid level {} in cookie_domains key '{}': level must be >= 1",
                level, pattern
            ));
        }

        let level_count = get_domain_level_count(pattern);
        if level_count > 0 && level > level_count {
            return Err(format!(
                "Invalid level {} in cookie_domains key '{}': level exceeds current domain levels ({})",
                level, pattern, level_count
            ));
        }

        let re = Regex::new(pattern)
            .map_err(|e| format!("Invalid regex in cookie_domains key '{}': {}", pattern, e))?;
        compiled_cookie_domains.push((re, level, pattern.clone()));
    }

    let mut compiled_acl_rules = HashMap::new();
    for (rule_name, rule) in &config.acl_rules {
        let mut compiled_allow = Vec::new();
        for (idx, m) in rule.allow.iter().enumerate() {
            let method_re = Regex::new(&format!("(?i){}", m.method)).map_err(|e| {
                format!(
                    "Rule '{}' allow[{}] invalid method regex '{}': {}",
                    rule_name, idx, m.method, e
                )
            })?;
            let domain_re = Regex::new(&m.domain).map_err(|e| {
                format!(
                    "Rule '{}' allow[{}] invalid domain regex '{}': {}",
                    rule_name, idx, m.domain, e
                )
            })?;
            let path_re = Regex::new(&m.path).map_err(|e| {
                format!(
                    "Rule '{}' allow[{}] invalid path regex '{}': {}",
                    rule_name, idx, m.path, e
                )
            })?;
            compiled_allow.push(CompiledMatchRule {
                method_re,
                domain_re,
                path_re,
            });
        }

        let mut compiled_deny = Vec::new();
        for (idx, m) in rule.deny.iter().enumerate() {
            let method_re = Regex::new(&format!("(?i){}", m.method)).map_err(|e| {
                format!(
                    "Rule '{}' deny[{}] invalid method regex '{}': {}",
                    rule_name, idx, m.method, e
                )
            })?;
            let domain_re = Regex::new(&m.domain).map_err(|e| {
                format!(
                    "Rule '{}' deny[{}] invalid domain regex '{}': {}",
                    rule_name, idx, m.domain, e
                )
            })?;
            let path_re = Regex::new(&m.path).map_err(|e| {
                format!(
                    "Rule '{}' deny[{}] invalid path regex '{}': {}",
                    rule_name, idx, m.path, e
                )
            })?;
            compiled_deny.push(CompiledMatchRule {
                method_re,
                domain_re,
                path_re,
            });
        }

        compiled_acl_rules.insert(
            rule_name.clone(),
            CompiledAclRule {
                allow: compiled_allow,
                deny: compiled_deny,
            },
        );
    }

    let compiled = CompiledAclConfig {
        cookie_domains: compiled_cookie_domains,
        acl_rules: compiled_acl_rules,
    };

    Ok((config, compiled))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acl_evaluation_deny_priority() {
        let yaml = r#"
acl_rules:
  test_rule:
    allow:
      - method: ".*"
        domain: ".*"
        path: ".*"
    deny:
      - method: "DELETE"
        domain: ".*"
        path: ".*"
      - method: ".*"
        domain: ".*"
        path: "^/admin/.*$"
"#;

        let (_, compiled) = parse_and_validate_yaml(yaml).expect("Failed to parse YAML");

        // GET /api/users -> Allowed
        assert!(compiled.evaluate("test_rule", "GET", "example.com", "/api/users"));

        // DELETE /api/users -> Denied (by method deny)
        assert!(!compiled.evaluate("test_rule", "DELETE", "example.com", "/api/users"));

        // GET /admin/dashboard -> Denied (by path deny)
        assert!(!compiled.evaluate("test_rule", "GET", "example.com", "/admin/dashboard"));

        // Unknown rule -> Denied
        assert!(!compiled.evaluate("non_existent", "GET", "example.com", "/"));
    }

    #[test]
    fn test_cookie_domain_resolution() {
        let yaml = r#"
cookie_domains:
  "^.*\\.b\\.a\\.com$": 1
"#;

        let (_, compiled) = parse_and_validate_yaml(yaml).expect("Failed to parse YAML");

        // sub.b.a.com -> 1 level up from b.a.com = a.com
        assert_eq!(
            compiled.resolve_cookie_domain("sub.b.a.com"),
            Some("a.com".to_string())
        );

        // unknown.com -> None (exact host match)
        assert_eq!(compiled.resolve_cookie_domain("unknown.com"), None);
    }

    #[test]
    fn test_cookie_domain_validation_rules() {
        // Test level < 1 fails
        let yaml_zero = r#"
cookie_domains:
  "^.*\\.b\\.a\\.com$": 0
"#;
        let res_zero = parse_and_validate_yaml(yaml_zero);
        assert!(res_zero.is_err());
        assert!(res_zero.unwrap_err().contains("level must be >= 1"));

        // Test level > current domain levels fails
        let yaml_excess = r#"
cookie_domains:
  "^.*\\.b\\.a\\.com$": 5
"#;
        let res_excess = parse_and_validate_yaml(yaml_excess);
        assert!(res_excess.is_err());
        assert!(res_excess
            .unwrap_err()
            .contains("exceeds current domain levels"));

        // Test string fails serde parse
        let yaml_string = r#"
cookie_domains:
  "^.*\\.b\\.a\\.com$": "a.com"
"#;
        let res_string = parse_and_validate_yaml(yaml_string);
        assert!(res_string.is_err());
        assert!(res_string.unwrap_err().contains("YAML parse error"));
    }

    #[test]
    fn test_empty_yaml_defaults() {
        let (config, compiled) = parse_and_validate_yaml("").expect("Failed to parse empty YAML");
        assert!(config.acl_rules.contains_key("allow_all"));
        assert!(config.acl_rules.contains_key("deny_all"));

        // allow_all allows everything
        assert!(compiled.evaluate("allow_all", "GET", "example.com", "/foo"));

        // deny_all denies everything
        assert!(!compiled.evaluate("deny_all", "GET", "example.com", "/foo"));
    }

    #[test]
    fn test_yaml_validation_errors() {
        let invalid_yaml = r#"
acl_rules:
  bad_rule:
    allow:
      - method: "("
"#;

        let res = parse_and_validate_yaml(invalid_yaml);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("invalid method regex"));
    }
}
