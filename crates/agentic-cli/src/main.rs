use std::path::PathBuf;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::SurrealStore;
use agentic_test_builder::{TestBuilder, TestBuilderError};
use agentic_uat::{StubExecutor, Uat, UatError, Verdict};
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

        /// Path to the store
        #[arg(long)]
        store: Option<PathBuf>,
    },
    /// Scaffold failing tests for a story and record red-state evidence
    #[command(name = "test-build")]
    TestBuild {
        /// Story ID to scaffold
        id: u32,
    },
}

#[derive(Subcommand)]
enum StoriesSubcommand {
    /// Display story health
    Health {
        /// Optional story ID for drilldown mode
        id: Option<u32>,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Path to the store
        #[arg(long)]
        store: Option<PathBuf>,
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
            StoriesSubcommand::Health { id, json, store } => {
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
                    Ok(r) => r.workdir().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from(".")),
                    Err(e) => {
                        eprintln!("failed to discover git repo: {e}");
                        std::process::exit(2);
                    }
                };

                let dashboard = Dashboard::with_repo(Arc::new(store), stories_dir, repo_root);

                let output = if let Some(story_id) = id {
                    match dashboard.drilldown(story_id) {
                        Ok(output) => output,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    }
                } else if json {
                    match dashboard.render_json() {
                        Ok(output) => output,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(2);
                        }
                    }
                } else {
                    match dashboard.render_table() {
                        Ok(output) => output,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(2);
                        }
                    }
                };

                println!("{output}");
            }
        },
        Commands::Uat { id, verdict, store } => {
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

            match uat.run(id) {
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
                Err(e) => {
                    eprintln!("uat failed: {e}");
                    std::process::exit(2);
                }
            }
        }
        Commands::TestBuild { id } => {
            let repo_root = match git2::Repository::discover(".") {
                Ok(r) => r.workdir().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from(".")),
                Err(e) => {
                    eprintln!("failed to discover git repo: {e}");
                    std::process::exit(2);
                }
            };

            let builder = TestBuilder::new(&repo_root);
            match builder.run(id) {
                Ok(outcome) => {
                    let created = outcome.created_paths();
                    let added = outcome.added_dev_deps();
                    println!("test-build {id}: {} created, {} dev-dep(s) added", created.len(), added.len());
                    for path in created {
                        println!("  CREATED {}", path.display());
                    }
                    for (crate_name, dep) in added {
                        println!("  DEV-DEP {crate_name} += {dep}");
                    }
                    std::process::exit(0);
                }
                Err(TestBuilderError::DirtyTree) => {
                    eprintln!("DirtyTree: working tree has uncommitted or untracked changes");
                    std::process::exit(2);
                }
                Err(TestBuilderError::NoAcceptanceTests) => {
                    eprintln!("NoAcceptanceTests: story has zero acceptance tests");
                    std::process::exit(2);
                }
                Err(TestBuilderError::ThinJustification { index }) => {
                    eprintln!("ThinJustification: acceptance.tests[{index}] has a thin justification");
                    std::process::exit(2);
                }
                Err(TestBuilderError::OutOfScopeEdit) => {
                    eprintln!("OutOfScopeEdit: scaffold requested a non-dev-dependency mutation");
                    std::process::exit(2);
                }
                Err(TestBuilderError::Other(msg)) => {
                    eprintln!("test-build failed: {msg}");
                    std::process::exit(2);
                }
            }
        }
    }
}

fn get_head_sha() -> Result<String, Box<dyn std::error::Error>> {
    let repo = git2::Repository::discover(".")?;
    let head = repo.head()?;
    let commit = head.peel_to_commit()?;
    Ok(commit.id().to_string())
}
