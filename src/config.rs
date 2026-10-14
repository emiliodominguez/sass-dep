//! Project configuration (`.sass-dep.toml`).
//!
//! Every setting is optional; command-line flags always take precedence
//! over the configuration file. Relative paths are resolved against the
//! project root (`--root`).
//!
//! ```toml
//! entry_points = ["src/main.scss", "src/admin.scss"]
//! load_paths = ["node_modules"]
//! extensions = ["scss", "sass"]
//! exclude = ["**/legacy/**"]          # skipped during orphan discovery
//! include_orphans = true
//!
//! [thresholds]
//! high_fan_in = 5
//! high_fan_out = 10
//!
//! [check]
//! no_cycles = true
//! max_depth = 10
//! no_unresolved = true
//! no_legacy_imports = true
//!
//! [[rules]]
//! name = "components are independent of pages"
//! from = "src/components/**"
//! deny = ["src/pages/**"]
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Deserializer};
use thiserror::Error;

use crate::analyzer::FlagThresholds;

/// Default configuration file name.
pub const DEFAULT_CONFIG_FILE: &str = ".sass-dep.toml";

/// An invalid configuration (exit code 2).
#[derive(Debug, Error)]
#[error("{0}")]
pub struct ConfigError(pub String);

/// Parsed `.sass-dep.toml`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Entry points used when none are given on the command line.
    pub entry_points: Vec<PathBuf>,
    /// Additional Sass load paths.
    pub load_paths: Vec<PathBuf>,
    /// File extensions to resolve, in priority order.
    pub extensions: Option<Vec<String>>,
    /// Globs (matched against root-relative IDs) skipped during orphan discovery.
    pub exclude: Vec<String>,
    /// Discover files unreachable from the entry points.
    pub include_orphans: bool,
    /// Thresholds for `high_fan_in` / `high_fan_out` flags.
    pub thresholds: FlagThresholds,
    /// Default constraints for `sass-dep check`.
    pub check: CheckConfig,
    /// Architecture rules enforced by `sass-dep check`.
    pub rules: Vec<RuleConfig>,
}

/// Constraints for `sass-dep check`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CheckConfig {
    /// Fail on circular dependencies.
    pub no_cycles: bool,
    /// Maximum allowed depth.
    pub max_depth: Option<usize>,
    /// Maximum allowed fan-out.
    pub max_fan_out: Option<usize>,
    /// Maximum allowed fan-in.
    pub max_fan_in: Option<usize>,
    /// Fail on files unreachable from every entry point.
    pub no_orphans: bool,
    /// Fail on imports that cannot be resolved.
    pub no_unresolved: bool,
    /// Fail on legacy `@import` usage.
    pub no_legacy_imports: bool,
}

/// A forbidden-dependency rule.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleConfig {
    /// Human-readable rule name used in reports.
    pub name: Option<String>,
    /// Globs selecting the importing files the rule applies to.
    #[serde(deserialize_with = "one_or_many")]
    pub from: Vec<String>,
    /// Globs of files that matching files must not import.
    #[serde(deserialize_with = "one_or_many")]
    pub deny: Vec<String>,
    /// Exceptions to `deny`.
    #[serde(default, deserialize_with = "one_or_many")]
    pub allow: Vec<String>,
    /// Extra explanation appended to violation messages.
    pub message: Option<String>,
}

/// A rule with compiled glob matchers.
#[derive(Debug, Clone)]
pub struct CompiledRule {
    /// Display name of the rule.
    pub name: String,
    /// Optional explanation.
    pub message: Option<String>,
    from: GlobSet,
    deny: GlobSet,
    allow: GlobSet,
}

impl CompiledRule {
    /// Returns `true` if an edge `from -> to` violates this rule.
    pub fn is_violated_by(&self, from: &str, to: &str) -> bool {
        self.from.is_match(from) && self.deny.is_match(to) && !self.allow.is_match(to)
    }
}

impl Config {
    /// Loads the configuration for a project.
    ///
    /// If `explicit` is `None`, `<root>/.sass-dep.toml` is used when it
    /// exists and defaults otherwise. An explicitly requested file must exist.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigError`] if the file is missing (explicit only) or invalid.
    pub fn load(root: &Path, explicit: Option<&Path>) -> Result<(Self, Option<PathBuf>)> {
        let path = match explicit {
            Some(path) => {
                let path = if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    root.join(path)
                };
                if !path.is_file() {
                    return Err(
                        ConfigError(format!("Config file not found: {}", path.display())).into(),
                    );
                }
                path
            }
            None => {
                let path = root.join(DEFAULT_CONFIG_FILE);
                if !path.is_file() {
                    return Ok((Self::default(), None));
                }
                path
            }
        };

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config: {}", path.display()))?;
        let config = Self::parse(&content)
            .map_err(|e| ConfigError(format!("Invalid config {}: {}", path.display(), e.0)))?;
        Ok((config, Some(path)))
    }

    /// Parses configuration from TOML text.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigError`] describing the first problem found.
    pub fn parse(content: &str) -> std::result::Result<Self, ConfigError> {
        let config: Self =
            toml::from_str(content).map_err(|e| ConfigError(e.message().to_string()))?;
        config.exclude_set()?;
        config.compiled_rules()?;
        Ok(config)
    }

    /// Compiles the `exclude` globs.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigError`] for an invalid glob.
    pub fn exclude_set(&self) -> std::result::Result<GlobSet, ConfigError> {
        build_globset(&self.exclude)
    }

    /// Compiles the architecture rules.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigError`] for an invalid glob or an empty rule.
    pub fn compiled_rules(&self) -> std::result::Result<Vec<CompiledRule>, ConfigError> {
        self.rules
            .iter()
            .enumerate()
            .map(|(i, rule)| {
                if rule.from.is_empty() || rule.deny.is_empty() {
                    return Err(ConfigError(format!(
                        "rules[{i}] needs both `from` and `deny`"
                    )));
                }
                Ok(CompiledRule {
                    name: rule
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("rule #{}", i + 1)),
                    message: rule.message.clone(),
                    from: build_globset(&rule.from)?,
                    deny: build_globset(&rule.deny)?,
                    allow: build_globset(&rule.allow)?,
                })
            })
            .collect()
    }
}

/// Builds a glob set where `*` does not cross `/` and `**` does.
fn build_globset(patterns: &[String]) -> std::result::Result<GlobSet, ConfigError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .map_err(|e| ConfigError(format!("invalid glob `{pattern}`: {e}")))?;
        builder.add(glob);
    }
    builder
        .build()
        .map_err(|e| ConfigError(format!("invalid glob set: {e}")))
}

/// Accepts either a single string or an array of strings.
fn one_or_many<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }

    Ok(match OneOrMany::deserialize(deserializer)? {
        OneOrMany::One(value) => vec![value],
        OneOrMany::Many(values) => values,
    })
}

/// Encodes `value` as a TOML basic string, escaping quotes, backslashes and
/// control characters so a file name can never break out of the string.
fn toml_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Starter configuration written by `sass-dep init`.
pub fn starter_config(entry_points: &[String]) -> String {
    let entries = if entry_points.is_empty() {
        "# entry_points = [\"src/main.scss\"]".to_string()
    } else {
        let list: Vec<String> = entry_points
            .iter()
            .map(|e| format!("    {},", toml_string(e)))
            .collect();
        format!("entry_points = [\n{}\n]", list.join("\n"))
    };

    format!(
        r#"# sass-dep configuration — https://github.com/emiliodominguez/sass-dep
# Command-line flags override these values. Paths are relative to this file's project root.

{entries}

# Extra directories searched by @use/@forward/@import (like Sass --load-path).
load_paths = []

# Also report files that are not reachable from any entry point.
include_orphans = true

# Globs skipped during orphan discovery (node_modules and .gitignore'd files are always skipped).
exclude = []

[thresholds]
high_fan_in = 5
high_fan_out = 10

# Constraints enforced by `sass-dep check`.
[check]
no_cycles = true
no_unresolved = true
# no_orphans = true
# no_legacy_imports = true
# max_depth = 10
# max_fan_out = 15
# max_fan_in = 30

# Architecture rules: files matching `from` must not import files matching `deny`.
# [[rules]]
# name = "components do not depend on pages"
# from = "src/components/**"
# deny = ["src/pages/**"]
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_config() {
        let config = Config::parse(
            r#"
entry_points = ["src/main.scss"]
load_paths = ["node_modules"]
exclude = ["**/legacy/**"]

[thresholds]
high_fan_in = 3

[check]
no_cycles = true
max_depth = 4

[[rules]]
from = "src/components/**"
deny = "src/pages/**"
allow = ["src/pages/_tokens.scss"]
"#,
        )
        .unwrap();

        assert_eq!(config.thresholds.high_fan_in, 3);
        assert_eq!(config.thresholds.high_fan_out, 10);
        assert!(config.check.no_cycles);
        assert_eq!(config.check.max_depth, Some(4));

        let rules = config.compiled_rules().unwrap();
        assert!(rules[0].is_violated_by("src/components/_card.scss", "src/pages/_home.scss"));
        assert!(!rules[0].is_violated_by("src/components/_card.scss", "src/pages/_tokens.scss"));
        assert!(!rules[0].is_violated_by("src/pages/_home.scss", "src/pages/_x.scss"));
    }

    #[test]
    fn single_star_does_not_cross_directories() {
        let config =
            Config::parse("[[rules]]\nfrom = \"src/*.scss\"\ndeny = \"vendor/**\"\n").unwrap();
        let rules = config.compiled_rules().unwrap();
        assert!(rules[0].is_violated_by("src/main.scss", "vendor/a/_b.scss"));
        assert!(!rules[0].is_violated_by("src/nested/main.scss", "vendor/a/_b.scss"));
    }

    #[test]
    fn rejects_unknown_keys_and_bad_globs() {
        assert!(Config::parse("entry_point = []").is_err());
        assert!(Config::parse("exclude = [\"src/[\"]").is_err());
        assert!(Config::parse("[[rules]]\nfrom = []\ndeny = \"x\"\n").is_err());
    }

    #[test]
    fn starter_config_escapes_hostile_names() {
        let names = vec![
            "we\"ird.scss".to_string(),
            "z\",]\nextensions = [\"sass\", \"x.scss".to_string(),
            "back\\slash\t\u{1b}[31m.scss".to_string(),
        ];
        let config = Config::parse(&starter_config(&names)).unwrap();
        let parsed: Vec<String> = config
            .entry_points
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(parsed, names);
        assert_eq!(config.extensions, None);
    }

    #[test]
    fn starter_config_parses() {
        let text = starter_config(&["src/main.scss".to_string()]);
        let config = Config::parse(&text).unwrap();
        assert_eq!(config.entry_points, vec![PathBuf::from("src/main.scss")]);
        assert!(config.check.no_cycles);
    }
}
