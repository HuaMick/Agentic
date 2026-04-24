//! # agentic-signer
//!
//! Resolves a signer identity string from a four-tier chain (flag → env
//! var → git config → typed error) and hands it to the evidence-writing
//! paths (`agentic-uat`, `agentic-ci-record`, `agentic-runtime`).
//!
//! ## Core resolution flow
//!
//! The `Signer::resolve(Resolver)` function walks the four tiers in strict order:
//!
//! 1. **Flag/explicit value** — passed via `Resolver::with_flag(s)`. Takes
//!    priority over every other source. Rejects empty/whitespace-only.
//!
//! 2. **`AGENTIC_SIGNER` environment variable** — consulted only when tier 1
//!    is absent. Rejects empty/whitespace-only.
//!
//! 3. **`git config user.email`** — consulted only when tiers 1 and 2 are absent.
//!    Read through `git2::Repository::config()` for repo-scoped + global fallthrough.
//!    Rejects empty/whitespace-only.
//!
//! 4. **`SignerError::SignerMissing`** — typed error naming the sources consulted.
//!    No further fallback (no unix user, no hostname, no `"unknown"`).
//!
//! ## Validation
//!
//! Validation applies at every tier: a tier-1 value of `""` or `"   "` is
//! rejected with `SignerError::SignerInvalid { source, reason }` rather than
//! falling through to tier 2. The failure is "this source was present but
//! unusable" — the resolver does not assume an empty value means skip this tier.
//!
//! The validation rule is: **trimmed length > 0**. No email-shape regex, no
//! maximum-length cap, no domain whitelist, no rejection of unicode or special
//! characters (including `:`). The sandbox convention `"sandbox:<model>@<run_id>"`
//! MUST pass validation.

use std::fmt;
use std::path::Path;

/// A non-empty, attributed identity string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signer(String);

impl Signer {
    /// Construct a `Signer` from a pre-validated string.
    /// This is private — use `Signer::resolve(Resolver)` to construct one.
    fn new(s: String) -> Self {
        Signer(s)
    }

    /// Resolve a signer identity from the given resolver config.
    ///
    /// Walks the four-tier chain (flag → env → git config → error) and
    /// returns the first non-empty, non-whitespace value found, or a typed
    /// error if all tiers are exhausted.
    pub fn resolve(resolver: Resolver) -> Result<Self, SignerError> {
        resolver.resolve()
    }

    /// Return the signer string as a `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Signer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Source of a signer value (flag, env var, or git config).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Explicit flag or constructor argument.
    Flag,
    /// `AGENTIC_SIGNER` environment variable.
    Env,
    /// `git config user.email`.
    Git,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Flag => write!(f, "flag"),
            Source::Env => write!(f, "env"),
            Source::Git => write!(f, "git"),
        }
    }
}

/// Reason a signer value was invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidReason {
    /// The value is empty.
    Empty,
    /// The value contains only whitespace.
    WhitespaceOnly,
}

impl fmt::Display for InvalidReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InvalidReason::Empty => write!(f, "empty"),
            InvalidReason::WhitespaceOnly => write!(f, "whitespace-only"),
        }
    }
}

/// Errors the signer resolver can produce.
#[derive(Debug)]
pub enum SignerError {
    /// No signer value was found in any of the consulted sources.
    /// The error names which sources were checked.
    SignerMissing { consulted: Vec<Source> },
    /// A signer value was found but was invalid (empty or whitespace-only).
    /// The error names the offending source and the reason.
    SignerInvalid {
        source: Source,
        reason: InvalidReason,
    },
    /// Git2 encountered an error while reading config (not "user.email is unset",
    /// but a genuine config-file-read error).
    GitConfigRead { source: git2::Error },
}

impl fmt::Display for SignerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignerError::SignerMissing { consulted } => {
                write!(f, "signer could not be resolved; consulted sources: ")?;
                for (i, src) in consulted.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", src)?;
                }
                Ok(())
            }
            SignerError::SignerInvalid { source, reason } => {
                write!(f, "signer from {source} is invalid: {reason}")
            }
            SignerError::GitConfigRead { source } => {
                write!(f, "git config read error: {source}")
            }
        }
    }
}

impl std::error::Error for SignerError {}

/// Configures the signer resolver.
///
/// Start with `Resolver::new()` for env-or-git resolution, or
/// `Resolver::with_flag(s)` for explicit-flag resolution.
/// Then call `.at_repo(path)` to set the repository path for git config reading.
#[derive(Debug)]
pub struct Resolver {
    flag: Option<String>,
    repo_path: Option<std::path::PathBuf>,
}

impl Resolver {
    /// Create a new resolver with no explicit flag.
    /// The resolver will consult the env var and git config.
    pub fn new() -> Self {
        Resolver {
            flag: None,
            repo_path: None,
        }
    }

    /// Create a resolver with an explicit flag value.
    /// The flag (if non-empty/non-whitespace) takes priority over env and git.
    pub fn with_flag(s: impl Into<String>) -> Self {
        Resolver {
            flag: Some(s.into()),
            repo_path: None,
        }
    }

    /// Set the repository path for git config reading.
    /// If not set, git resolution will be skipped.
    pub fn at_repo(mut self, path: impl AsRef<Path>) -> Self {
        self.repo_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Resolve the signer identity from this resolver config.
    /// Walks the four-tier chain and returns a `Signer` or a typed error.
    fn resolve(self) -> Result<Signer, SignerError> {
        // Tier 1: explicit flag.
        if let Some(flag_val) = self.flag {
            let trimmed = flag_val.trim();
            if !trimmed.is_empty() {
                return Ok(Signer::new(flag_val));
            } else {
                // Flag was empty or whitespace-only.
                let reason = if flag_val.is_empty() {
                    InvalidReason::Empty
                } else {
                    InvalidReason::WhitespaceOnly
                };
                return Err(SignerError::SignerInvalid {
                    source: Source::Flag,
                    reason,
                });
            }
        }

        let mut consulted = vec![Source::Flag];

        // Tier 2: environment variable.
        if let Ok(env_val) = std::env::var("AGENTIC_SIGNER") {
            let trimmed = env_val.trim();
            if !trimmed.is_empty() {
                return Ok(Signer::new(env_val));
            } else {
                // Env var was empty or whitespace-only.
                let reason = if env_val.is_empty() {
                    InvalidReason::Empty
                } else {
                    InvalidReason::WhitespaceOnly
                };
                return Err(SignerError::SignerInvalid {
                    source: Source::Env,
                    reason,
                });
            }
        }
        consulted.push(Source::Env);

        // Tier 3: git config user.email.
        // Read from repo's local config only (not global/system). This allows tests
        // to isolate their config and not be affected by the developer's global
        // ~/.gitconfig. This also matches the intent of "repo-scoped" lookups.
        if let Some(repo_path) = self.repo_path {
            if let Ok(repo) = git2::Repository::discover(&repo_path) {
                // Open the repo's local config file only (not the full chain).
                let config_path = repo.path().join("config");
                if config_path.exists() {
                    if let Ok(config) = git2::Config::open(&config_path) {
                        match config.get_string("user.email") {
                            Ok(git_val) => {
                                let trimmed = git_val.trim();
                                if !trimmed.is_empty() {
                                    return Ok(Signer::new(git_val));
                                } else {
                                    let reason = if git_val.is_empty() {
                                        InvalidReason::Empty
                                    } else {
                                        InvalidReason::WhitespaceOnly
                                    };
                                    return Err(SignerError::SignerInvalid {
                                        source: Source::Git,
                                        reason,
                                    });
                                }
                            }
                            Err(_) => {
                                // user.email not set in repo config — continue to tier 4.
                            }
                        }
                    }
                }
            }
        }
        consulted.push(Source::Git);

        // Tier 4: all sources exhausted.
        Err(SignerError::SignerMissing { consulted })
    }
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}
