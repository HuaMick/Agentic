use std::path::PathBuf;
use std::sync::Arc;

use agentic_ci_record::{ExecutorOutcome, TestExecutor};
use agentic_dashboard::Dashboard;
use agentic_store::SurrealStore;
use agentic_story::Story;
use agentic_test_builder::{TestBuilder, TestBuilderError};
use agentic_uat::{SignerSource, StubExecutor, Uat, UatError, Verdict};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "agentic")]
#[command(about = "Wire the agentic binary to run UATs and read the dashboard")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Interact with story health and the dashboard
    Stories {
        #[command(subcommand)]
        subcommand: StoriesSubcommand,
    },
    /// Run a UAT on a story
    Uat {
        /// Story ID to run UAT on
        id: u32,

        /// Verdict: pass or fail
        #[arg(long, value_parser = parse_verdict)]
        verdict: Option<Verdict>,

        /// Signer identity (overrides env var and git config)
        #[arg(long)]
        signer: Option<String>,

        /// Path to the store
        #[arg(long)]
        store: Option<PathBuf>,
    },
    /// Scaffold failing tests for a story and record red-state evidence
    #[command(name = "test-build")]
    TestBuild {
        #[command(subcommand)]
        subcommand: Option<TestBuildSubcommand>,

        /// Story ID to operate on (when no subcommand is given, alias to `plan`)
        id: Option<u32>,
    },
}

#[derive(Subcommand)]
enum StoriesSubcommand {
    /// Display story health
    Health {
        /// Optional selector: <id> (drilldown), +<id> (ancestors), <id>+ (descendants), +<id>+ (subtree)
        selector: Option<String>,

        /// Show full not-healthy subtree (mutually exclusive with selector and --all)
        #[arg(long)]
        expand: bool,

        /// Show all stories including healthy (mutually exclusive with selector and --expand)
        #[arg(long)]
        all: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Path to the store
        #[arg(long)]
        store: Option<PathBuf>,
    },
    /// Run tests on stories matching a selector
    Test {
        /// Selector: <id> (single story), +<id> (ancestors), <id>+ (descendants), +<id>+ (subtree)
        selector: String,

        /// Path to the store
        #[arg(long)]
        store: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum TestBuildSubcommand {
    /// Plan the scaffolds for a story (default mode, no subcommand)
    Plan {
        /// Story ID
        id: u32,

        /// Output as JSON (default is human-readable text)
        #[arg(long)]
        json: bool,
    },
    /// Record red-state evidence for user-authored scaffolds
    Record {
        /// Story ID
        id: u32,
    },
}

fn parse_verdict(s: &str) -> Result<Verdict, String> {
    match s {
        "pass" => Ok(Verdict::Pass),
        "fail" => Ok(Verdict::Fail),
        _ => Err(format!("verdict must be 'pass' or 'fail', got '{s}'")),
    }
}

fn resolve_store_path(explicit_path: Option<PathBuf>) -> PathBuf {
    explicit_path
        .or_else(|| std::env::var("AGENTIC_STORE").ok().map(PathBuf::from))
        .unwrap_or_else(|| {
            // Default: $XDG_DATA_HOME/agentic/store or $HOME/.local/share/agentic/store on Unix,
            // or dirs::data_dir().join("agentic/store") on Windows.
            dirs::data_dir()
                .map(|d| d.join("agentic").join("store"))
                .unwrap_or_else(|| {
                    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
                        .join(".local/share/agentic/store")
                })
        })
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Stories { subcommand } => match subcommand {
            StoriesSubcommand::Health {
                selector,
                expand,
                all,
                json,
                store,
            } => {
                // Validate mutually exclusive flags
                let selector_provided = selector.is_some();
                if all && (selector_provided || expand) {
                    eprintln!("--all is mutually exclusive with positional selector and --expand");
                    std::process::exit(2);
                }
                if expand && selector_provided {
                    eprintln!("--expand is mutually exclusive with positional selector");
                    std::process::exit(2);
                }

                let store_path = resolve_store_path(store);
                eprintln!("store: {}", store_path.display());

                let store = match SurrealStore::open(&store_path) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("failed to open store: {e}");
                        std::process::exit(2);
                    }
                };

                let stories_dir = PathBuf::from("stories");
                if !stories_dir.exists() {
                    eprintln!("stories directory not found");
                    std::process::exit(2);
                }

                let repo_root = match git2::Repository::discover(".") {
                    Ok(r) => r
                        .workdir()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| PathBuf::from(".")),
                    Err(e) => {
                        eprintln!("failed to discover git repo: {e}");
                        std::process::exit(2);
                    }
                };

                let dashboard = Dashboard::with_repo(Arc::new(store), stories_dir, repo_root);

                let output = if let Some(sel) = selector {
                    // Check if it's a drilldown (bareword) or a selector (+id, id+, +id+)
                    if sel.contains('+') {
                        // It's a selector
                        match dashboard.list_selector(&sel) {
                            Ok(output) => output,
                            Err(e) => {
                                eprintln!("{e}");
                                std::process::exit(1);
                            }
                        }
                    } else {
                        // It's a drilldown
                        match sel.parse::<u32>() {
                            Ok(story_id) => match dashboard.drilldown(story_id) {
                                Ok(output) => output,
                                Err(e) => {
                                    eprintln!("{e}");
                                    std::process::exit(1);
                                }
                            },
                            Err(_) => {
                                eprintln!("invalid selector: {}", sel);
                                std::process::exit(2);
                            }
                        }
                    }
                } else if expand {
                    if json {
                        match dashboard.render_expand_json() {
                            Ok(output) => output,
                            Err(e) => {
                                eprintln!("{e}");
                                std::process::exit(2);
                            }
                        }
                    } else {
                        match dashboard.render_expand_table() {
                            Ok(output) => output,
                            Err(e) => {
                                eprintln!("{e}");
                                std::process::exit(2);
                            }
                        }
                    }
                } else if all {
                    if json {
                        match dashboard.render_all_json() {
                            Ok(output) => output,
                            Err(e) => {
                                eprintln!("{e}");
                                std::process::exit(2);
                            }
                        }
                    } else {
                        match dashboard.render_all_table() {
                            Ok(output) => output,
                            Err(e) => {
                                eprintln!("{e}");
                                std::process::exit(2);
                            }
                        }
                    }
                } else {
                    // Default: frontier view
                    if json {
                        match dashboard.render_frontier_json() {
                            Ok(output) => output,
                            Err(e) => {
                                eprintln!("{e}");
                                std::process::exit(2);
                            }
                        }
                    } else {
                        match dashboard.render_frontier_table() {
                            Ok(output) => output,
                            Err(e) => {
                                eprintln!("{e}");
                                std::process::exit(2);
                            }
                        }
                    }
                };

                println!("{output}");
            }
            StoriesSubcommand::Test { selector, store } => {
                let store_path = resolve_store_path(store);
                eprintln!("store: {}", store_path.display());

                let store = match SurrealStore::open(&store_path) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("failed to open store: {e}");
                        std::process::exit(2);
                    }
                };

                let stories_dir = PathBuf::from("stories");
                if !stories_dir.exists() {
                    eprintln!("stories directory not found");
                    std::process::exit(2);
                }

                // Create the test executor (stub or real)
                let executor: Box<dyn agentic_ci_record::TestExecutor> =
                    if std::env::var("AGENTIC_CI_TEST_EXECUTOR").as_deref() == Ok("stub-pass") {
                        Box::new(StubTestExecutor)
                    } else {
                        Box::new(CargoTestExecutor)
                    };

                let runner =
                    agentic_ci_record::CiRunner::new(Arc::new(store), executor, stories_dir);

                match runner.run(&selector) {
                    Ok(0) => {
                        // All tests passed
                        std::process::exit(0);
                    }
                    Ok(1) => {
                        // Some tests failed
                        std::process::exit(1);
                    }
                    Ok(_) => {
                        // Should not happen
                        eprintln!("unexpected exit code from runner");
                        std::process::exit(2);
                    }
                    Err(agentic_ci_record::CiRunError::UnknownStory { id }) => {
                        eprintln!("unknown story id: {id}");
                        std::process::exit(2);
                    }
                    Err(agentic_ci_record::CiRunError::Cycle { participants: _ }) => {
                        eprintln!("cycle detected in depends_on graph");
                        std::process::exit(2);
                    }
                    Err(agentic_ci_record::CiRunError::BadSelector { input, reason }) => {
                        eprintln!("bad selector '{}': {}", input, reason);
                        std::process::exit(2);
                    }
                    Err(e) => {
                        eprintln!("runner error: {e}");
                        std::process::exit(2);
                    }
                }
            }
        },
        Commands::Uat {
            id,
            verdict,
            signer,
            store,
        } => {
            // Verdict is required — exit 2 if missing
            let verdict = match verdict {
                Some(v) => v,
                None => {
                    eprintln!("missing --verdict");
                    std::process::exit(2);
                }
            };

            let store_path = resolve_store_path(store);
            eprintln!("store: {}", store_path.display());

            let store = match SurrealStore::open(&store_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("could not open store: {e}");
                    std::process::exit(2);
                }
            };

            let stories_dir = PathBuf::from("stories");

            let executor = match verdict {
                Verdict::Pass => StubExecutor::always_pass(),
                Verdict::Fail => StubExecutor::always_fail(),
            };

            let uat = Uat::new(Arc::new(store), executor, stories_dir);

            // Resolve the signer: explicit flag takes priority, else resolve via chain
            let signer_source = match signer {
                Some(s) => SignerSource::Explicit(s),
                None => SignerSource::Resolve,
            };

            match uat.run(id, signer_source) {
                Ok(Verdict::Pass) => {
                    // Get the HEAD SHA to include in stdout
                    match get_head_sha() {
                        Ok(sha) => {
                            println!("pass {sha}");
                            std::process::exit(0);
                        }
                        Err(e) => {
                            eprintln!("failed to get HEAD SHA: {e}");
                            std::process::exit(2);
                        }
                    }
                }
                Ok(Verdict::Fail) => {
                    // Get the HEAD SHA to include in stdout
                    match get_head_sha() {
                        Ok(sha) => {
                            println!("fail {sha}");
                            std::process::exit(1);
                        }
                        Err(e) => {
                            eprintln!("failed to get HEAD SHA: {e}");
                            std::process::exit(2);
                        }
                    }
                }
                Err(UatError::DirtyTree) => {
                    eprintln!("dirty tree");
                    std::process::exit(2);
                }
                Err(UatError::UnknownStory { id }) => {
                    eprintln!("unknown story id: {id}");
                    std::process::exit(2);
                }
                Err(UatError::SignerMissing) => {
                    eprintln!("signer identity could not be resolved");
                    std::process::exit(2);
                }
                Err(UatError::AncestorNotHealthy {
                    ancestor_id,
                    reason,
                }) => {
                    eprintln!("ancestor {ancestor_id} is not healthy: {reason}");
                    std::process::exit(2);
                }
                Err(UatError::Cycle { edge: (from, to) }) => {
                    eprintln!("cycle in depends_on graph: {from} -> {to}");
                    std::process::exit(2);
                }
                Err(e) => {
                    eprintln!("uat failed: {e}");
                    std::process::exit(2);
                }
            }
        }
        Commands::TestBuild { subcommand, id } => {
            let repo_root = match git2::Repository::discover(".") {
                Ok(r) => r
                    .workdir()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from(".")),
                Err(e) => {
                    eprintln!("failed to discover git repo: {e}");
                    std::process::exit(2);
                }
            };

            // Default mode: if no subcommand, alias to `plan` with the id.
            let (mode, story_id, json_mode) = match (&subcommand, &id) {
                (Some(TestBuildSubcommand::Plan { id, json }), None) => ("plan", *id, *json),
                (Some(TestBuildSubcommand::Record { id }), None) => ("record", *id, false),
                (None, Some(story_id)) => ("plan", *story_id, false), // default to plan, text mode
                _ => {
                    eprintln!(
                        "invalid test-build invocation: provide either a subcommand or an id"
                    );
                    std::process::exit(2);
                }
            };

            match mode {
                "plan" => {
                    let story_path = repo_root.join(format!("stories/{story_id}.yml"));
                    let story = match Story::load(&story_path) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("failed to load story: {e}");
                            std::process::exit(2);
                        }
                    };

                    let plan = TestBuilder::plan(&story);

                    if json_mode {
                        match serde_json::to_string_pretty(&plan) {
                            Ok(json) => {
                                println!("{json}");
                                std::process::exit(0);
                            }
                            Err(e) => {
                                eprintln!("failed to serialize plan as JSON: {e}");
                                std::process::exit(2);
                            }
                        }
                    } else {
                        // Pretty-print text mode
                        for (i, entry) in plan.iter().enumerate() {
                            println!("[{}] {}", i, entry.file);
                            println!("    crate: {}", entry.target_crate);
                            println!("    expected: {}", entry.expected_red_path);
                            if !entry.fixture_preconditions.is_empty() {
                                println!("    preconditions:");
                                for precond in &entry.fixture_preconditions {
                                    println!("      - {precond}");
                                }
                            }
                            println!(
                                "    justification: {}",
                                entry.justification.lines().next().unwrap_or("")
                            );
                        }
                        std::process::exit(0);
                    }
                }
                "record" => {
                    let builder = TestBuilder::new(&repo_root);
                    match builder.record(story_id) {
                        Ok(outcome) => {
                            for path in outcome.recorded_paths() {
                                println!("{}", path.display());
                            }
                            std::process::exit(0);
                        }
                        Err(TestBuilderError::DirtyTree) => {
                            eprintln!("DirtyTree");
                            std::process::exit(2);
                        }
                        Err(TestBuilderError::NoAcceptanceTests) => {
                            eprintln!("NoAcceptanceTests");
                            std::process::exit(2);
                        }
                        Err(TestBuilderError::ThinJustification { index }) => {
                            eprintln!("ThinJustification: index {index}");
                            std::process::exit(2);
                        }
                        Err(TestBuilderError::ScaffoldMissing { file }) => {
                            eprintln!("ScaffoldMissing: {}", file.display());
                            std::process::exit(2);
                        }
                        Err(TestBuilderError::ScaffoldParseError { file, parse_error }) => {
                            eprintln!("ScaffoldParseError: {}: {}", file.display(), parse_error);
                            std::process::exit(2);
                        }
                        Err(TestBuilderError::ScaffoldNotRed { file, probe }) => {
                            eprintln!("ScaffoldNotRed: {} ({})", file.display(), probe);
                            std::process::exit(2);
                        }
                        Err(TestBuilderError::ClassificationFailed(msg)) => {
                            eprintln!("ClassificationFailed: {msg}");
                            std::process::exit(2);
                        }
                        Err(TestBuilderError::Other(msg)) => {
                            eprintln!("test-build record failed: {msg}");
                            std::process::exit(2);
                        }
                    }
                }
                _ => {
                    eprintln!("unknown test-build mode");
                    std::process::exit(2);
                }
            }
        }
    }
}

/// Stub test executor that always returns Pass.
struct StubTestExecutor;

impl TestExecutor for StubTestExecutor {
    fn run_tests(&self, _story_id: u32, _test_files: &[PathBuf]) -> ExecutorOutcome {
        ExecutorOutcome::pass()
    }
}

/// Real test executor that runs `cargo test --test <name>` for each test file.
struct CargoTestExecutor;

impl TestExecutor for CargoTestExecutor {
    fn run_tests(&self, _story_id: u32, test_files: &[PathBuf]) -> ExecutorOutcome {
        let mut failing_tests = Vec::new();

        for test_file in test_files {
            // Extract the test name from the file path (basename without extension)
            let test_name = test_file
                .file_stem()
                .and_then(|os| os.to_str())
                .unwrap_or_else(|| "unknown");

            // Run `cargo test --test <name>` for this test file
            let output = std::process::Command::new("cargo")
                .args(&["test", "--test", test_name])
                .output();

            match output {
                Ok(status) => {
                    if !status.status.success() {
                        failing_tests.push(test_name.to_string());
                    }
                }
                Err(_e) => {
                    // If cargo invocation fails, mark this test as failing
                    failing_tests.push(test_name.to_string());
                }
            }
        }

        if failing_tests.is_empty() {
            ExecutorOutcome::pass()
        } else {
            ExecutorOutcome::fail(failing_tests)
        }
    }
}

fn get_head_sha() -> Result<String, Box<dyn std::error::Error>> {
    let repo = git2::Repository::discover(".")?;
    let head = repo.head()?;
    let commit = head.peel_to_commit()?;
    Ok(commit.id().to_string())
}
