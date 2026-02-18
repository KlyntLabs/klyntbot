//! Database configuration wizard step.
//!
//! Guides users through installing, starting, and configuring PostgreSQL
//! for klyntbot's persistent storage layer. Detects the current system state,
//! shows a checklist of needed actions, and auto-installs everything:
//!
//! 1. Smart binary detection (PATH + Homebrew cellar + Linux apt paths)
//! 2. Dependency status assessment
//! 3. Checklist UI with spacebar selection
//! 4. Automated installation with spinner progress
//! 5. Connection test + migration
//! 6. Save `database_url` to config

use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;
use common::utils::terminal::*;

use crate::wizard::framework::{StepResult, WizardState};
use crate::wizard::prompts::{
    prompt_multi_select_with_defaults, prompt_text, prompt_yes_no, SelectOption,
};

/// Default PostgreSQL connection URL.
const DEFAULT_DATABASE_URL: &str = "postgresql://localhost/klyntbot";

/// Default database name to create.
const DEFAULT_DB_NAME: &str = "klyntbot";

// ============================================================================
// Smart binary detection
// ============================================================================

/// Discovered PostgreSQL binary paths.
struct PgPaths {
    psql: Option<PathBuf>,
    pg_isready: Option<PathBuf>,
    createdb: Option<PathBuf>,
}

impl PgPaths {
    /// True if all required binaries were found.
    fn is_complete(&self) -> bool {
        self.psql.is_some() && self.pg_isready.is_some() && self.createdb.is_some()
    }
}

/// Find a PostgreSQL binary by name, checking PATH then Homebrew/apt cellar paths.
fn find_pg_binary(name: &str) -> Option<PathBuf> {
    // 1. Check PATH via `which`
    if let Ok(output) = Command::new("which").arg(name).output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }

    // 2. Scan Homebrew cellar dirs (Apple Silicon + Intel)
    for base in &["/opt/homebrew/opt", "/usr/local/opt"] {
        if let Some(path) = scan_homebrew_cellar(base, name) {
            return Some(path);
        }
    }

    // 3. Scan Linux apt paths
    if let Some(path) = scan_linux_pg_dirs(name) {
        return Some(path);
    }

    None
}

/// Scan `/opt/homebrew/opt/postgresql@*/bin/{name}` for a binary.
fn scan_homebrew_cellar(base: &str, name: &str) -> Option<PathBuf> {
    let base_path = std::path::Path::new(base);
    if !base_path.exists() {
        return None;
    }

    let Ok(entries) = std::fs::read_dir(base_path) else {
        return None;
    };

    // Collect postgresql@* dirs, sort descending to prefer newest version
    let mut pg_dirs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("postgresql@"))
                .unwrap_or(false)
        })
        .collect();
    pg_dirs.sort();
    pg_dirs.reverse();

    for dir in pg_dirs {
        let bin = dir.join("bin").join(name);
        if bin.exists() {
            return Some(bin);
        }
    }

    None
}

/// Scan `/usr/lib/postgresql/*/bin/{name}` for a binary.
fn scan_linux_pg_dirs(name: &str) -> Option<PathBuf> {
    let base = std::path::Path::new("/usr/lib/postgresql");
    if !base.exists() {
        return None;
    }

    let Ok(entries) = std::fs::read_dir(base) else {
        return None;
    };

    let mut version_dirs: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    version_dirs.sort();
    version_dirs.reverse();

    for dir in version_dirs {
        let bin = dir.join("bin").join(name);
        if bin.exists() {
            return Some(bin);
        }
    }

    None
}

/// Discover all required PostgreSQL binaries.
fn discover_pg_paths() -> PgPaths {
    PgPaths {
        psql: find_pg_binary("psql"),
        pg_isready: find_pg_binary("pg_isready"),
        createdb: find_pg_binary("createdb"),
    }
}

// ============================================================================
// Dependency status detection
// ============================================================================

/// Full dependency status for the database setup.
struct DepStatus {
    has_homebrew: bool,
    has_apt: bool,
    pg_installed: bool,
    pg_running: bool,
    db_exists: bool,
    pgvector_pkg_installed: bool,
    pgvector_ext_enabled: bool,
}

/// Detect all dependency states upfront.
fn detect_status(paths: &PgPaths) -> DepStatus {
    let has_homebrew = Command::new("brew")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let has_apt = Command::new("apt-get")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let pg_installed = paths.is_complete();

    let pg_running = paths
        .pg_isready
        .as_ref()
        .map(|bin| {
            Command::new(bin)
                .arg("--quiet")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
        .unwrap_or(false);

    let db_exists = if pg_running {
        check_database_exists(paths, DEFAULT_DB_NAME)
    } else {
        false
    };

    let pgvector_pkg_installed = if db_exists {
        check_pgvector_available(paths, DEFAULT_DB_NAME)
    } else {
        false
    };

    let pgvector_ext_enabled = if db_exists {
        check_pgvector_enabled(paths, DEFAULT_DB_NAME)
    } else {
        false
    };

    DepStatus {
        has_homebrew,
        has_apt,
        pg_installed,
        pg_running,
        db_exists,
        pgvector_pkg_installed,
        pgvector_ext_enabled,
    }
}

/// Check if a database exists using discovered psql path.
fn check_database_exists(paths: &PgPaths, db_name: &str) -> bool {
    let Some(ref psql) = paths.psql else {
        return false;
    };
    Command::new(psql)
        .args(["-d", db_name, "-c", "SELECT 1"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if pgvector extension is available (package installed).
fn check_pgvector_available(paths: &PgPaths, db_name: &str) -> bool {
    let Some(ref psql) = paths.psql else {
        return false;
    };
    Command::new(psql)
        .args([
            "-d",
            db_name,
            "-t",
            "-c",
            "SELECT 1 FROM pg_available_extensions WHERE name = 'vector'",
        ])
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim().contains('1'))
        .unwrap_or(false)
}

/// Check if pgvector extension is enabled (CREATE EXTENSION done).
fn check_pgvector_enabled(paths: &PgPaths, db_name: &str) -> bool {
    let Some(ref psql) = paths.psql else {
        return false;
    };
    Command::new(psql)
        .args([
            "-d",
            db_name,
            "-t",
            "-c",
            "SELECT 1 FROM pg_extension WHERE extname = 'vector'",
        ])
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim().contains('1'))
        .unwrap_or(false)
}

// ============================================================================
// Checklist item types
// ============================================================================

/// Actions the wizard can perform automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    InstallPostgres,
    StartPostgres,
    CreateDatabase,
    InstallPgvector,
    EnablePgvector,
}

/// A checklist item with label, action, and whether it should be pre-checked.
struct ChecklistItem {
    label: &'static str,
    description: &'static str,
    action: Action,
}

/// Build the list of needed actions based on current status.
fn build_checklist(status: &DepStatus) -> Vec<ChecklistItem> {
    let mut items = Vec::new();

    if !status.pg_installed {
        items.push(ChecklistItem {
            label: "Install PostgreSQL 17",
            description: "via Homebrew / apt",
            action: Action::InstallPostgres,
        });
    }

    if !status.pg_running && status.pg_installed {
        items.push(ChecklistItem {
            label: "Start PostgreSQL service",
            description: "brew services start / systemctl start",
            action: Action::StartPostgres,
        });
    }

    if !status.db_exists {
        items.push(ChecklistItem {
            label: "Create 'klyntbot' database",
            description: "createdb klyntbot",
            action: Action::CreateDatabase,
        });
    }

    if !status.pgvector_pkg_installed {
        items.push(ChecklistItem {
            label: "Install pgvector",
            description: "for semantic search (optional)",
            action: Action::InstallPgvector,
        });
    }

    if status.pgvector_pkg_installed && !status.pgvector_ext_enabled {
        items.push(ChecklistItem {
            label: "Enable pgvector extension",
            description: "CREATE EXTENSION vector",
            action: Action::EnablePgvector,
        });
    }

    items
}

// ============================================================================
// Automated execution
// ============================================================================

/// Run a shell command, returning Ok(()) on success or Err(stderr) on failure.
fn run_command(cmd: &str, args: &[&str]) -> std::result::Result<(), String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run {}: {}", cmd, e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(stderr.trim().to_string())
    }
}

/// Run a shell command using a discovered binary path.
fn run_pg_command(bin: &PathBuf, args: &[&str]) -> std::result::Result<(), String> {
    let output = Command::new(bin)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run {}: {}", bin.display(), e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(stderr.trim().to_string())
    }
}

/// Execute a single action with spinner progress.
fn execute_action(
    action: Action,
    paths: &PgPaths,
    prefix: &str,
) -> std::result::Result<(), String> {
    match action {
        Action::InstallPostgres => {
            let mut spinner = Spinner::new("Installing PostgreSQL 17...");
            spinner.start();
            let result = if cfg!(target_os = "macos") {
                run_command("brew", &["install", "postgresql@17"])
            } else {
                run_command(
                    "sudo",
                    &[
                        "apt-get",
                        "install",
                        "-y",
                        "postgresql",
                        "postgresql-contrib",
                    ],
                )
            };
            spinner.stop();
            match &result {
                Ok(()) => println!(
                    "{}{} PostgreSQL 17 installed.",
                    prefix,
                    colorize("✓", SUCCESS),
                ),
                Err(e) => println!(
                    "{}{} Failed to install PostgreSQL: {}",
                    prefix,
                    colorize("✗", ERROR),
                    e,
                ),
            }
            result
        }
        Action::StartPostgres => {
            let mut spinner = Spinner::new("Starting PostgreSQL service...");
            spinner.start();
            let result = if cfg!(target_os = "macos") {
                run_command("brew", &["services", "start", "postgresql@17"])
            } else {
                run_command("sudo", &["systemctl", "start", "postgresql"])
            };
            spinner.stop();

            // Give PostgreSQL a moment to start accepting connections
            if result.is_ok() {
                std::thread::sleep(std::time::Duration::from_secs(2));
            }

            match &result {
                Ok(()) => println!(
                    "{}{} PostgreSQL is running.",
                    prefix,
                    colorize("✓", SUCCESS),
                ),
                Err(e) => println!(
                    "{}{} Failed to start PostgreSQL: {}",
                    prefix,
                    colorize("✗", ERROR),
                    e,
                ),
            }
            result
        }
        Action::CreateDatabase => {
            let mut spinner = Spinner::new("Creating database 'klyntbot'...");
            spinner.start();
            let result = if let Some(ref createdb) = paths.createdb {
                run_pg_command(createdb, &[DEFAULT_DB_NAME])
            } else {
                // Try PATH as fallback (may work after fresh install)
                run_command("createdb", &[DEFAULT_DB_NAME])
            };
            spinner.stop();
            match &result {
                Ok(()) => println!(
                    "{}{} Database '{}' created.",
                    prefix,
                    colorize("✓", SUCCESS),
                    DEFAULT_DB_NAME,
                ),
                Err(e) => println!(
                    "{}{} Failed to create database: {}",
                    prefix,
                    colorize("✗", ERROR),
                    e,
                ),
            }
            result
        }
        Action::InstallPgvector => {
            let mut spinner = Spinner::new("Installing pgvector...");
            spinner.start();
            let result = if cfg!(target_os = "macos") {
                run_command("brew", &["install", "pgvector"])
            } else {
                run_command(
                    "sudo",
                    &["apt-get", "install", "-y", "postgresql-17-pgvector"],
                )
            };
            spinner.stop();
            match &result {
                Ok(()) => println!("{}{} pgvector installed.", prefix, colorize("✓", SUCCESS),),
                Err(e) => println!(
                    "{}{} Failed to install pgvector: {}",
                    prefix,
                    colorize("✗", ERROR),
                    e,
                ),
            }
            result
        }
        Action::EnablePgvector => {
            let mut spinner = Spinner::new("Enabling pgvector extension...");
            spinner.start();
            let result = if let Some(ref psql) = paths.psql {
                run_pg_command(
                    psql,
                    &[
                        "-d",
                        DEFAULT_DB_NAME,
                        "-c",
                        "CREATE EXTENSION IF NOT EXISTS vector",
                    ],
                )
            } else {
                run_command(
                    "psql",
                    &[
                        "-d",
                        DEFAULT_DB_NAME,
                        "-c",
                        "CREATE EXTENSION IF NOT EXISTS vector",
                    ],
                )
            };
            spinner.stop();
            match &result {
                Ok(()) => println!(
                    "{}{} pgvector extension enabled.",
                    prefix,
                    colorize("✓", SUCCESS),
                ),
                Err(e) => println!(
                    "{}{} Failed to enable pgvector: {}",
                    prefix,
                    colorize("✗", ERROR),
                    e,
                ),
            }
            result
        }
    }
}

// ============================================================================
// Main wizard step
// ============================================================================

/// Run the guided database configuration step.
///
/// Detects PostgreSQL state on the system, shows a checklist of needed actions,
/// and auto-installs everything with spinner progress.
pub async fn run_database_step(state: &mut WizardState) -> Result<StepResult> {
    let prefix = step_prefix();

    // Show current state if already configured
    if let Some(ref url) = state.config.database_url {
        println!(
            "{}{} Database: {} ({})",
            prefix,
            colorize("✓", SUCCESS),
            colorize("PostgreSQL", BOLD),
            colorize(&mask_url(url), DIM),
        );
        println!("{}", prefix);

        let reconfigure = prompt_yes_no("Reconfigure database?", false)?;
        if !reconfigure {
            return Ok(StepResult::Next);
        }
    }

    println!(
        "{}{}",
        prefix,
        colorize(
            "klyntbot uses PostgreSQL for all persistent data (tasks, sessions, plans, etc.).",
            DIM,
        )
    );
    println!("{}{}", prefix, colorize("Checking your system...", DIM));
    println!("{}", prefix);

    // ── Discover binaries + detect status ────────────────────────────────
    let mut paths = discover_pg_paths();
    let status = detect_status(&paths);

    // If no package manager, fall back to manual
    if !status.has_homebrew && !status.has_apt && !status.pg_installed {
        println!(
            "{}{} PostgreSQL not found and no supported package manager detected.",
            prefix,
            colorize("!", WARNING),
        );
        println!(
            "{}{}",
            prefix,
            colorize("Install PostgreSQL manually or enter a database URL.", DIM),
        );
        println!("{}", prefix);
        return manual_url_entry(state).await;
    }

    // ── Show current status ─────────────────────────────────────────────
    print_status_overview(&prefix, &status);

    // ── Build checklist of needed actions ────────────────────────────────
    let checklist = build_checklist(&status);

    if checklist.is_empty() {
        // Everything is ready — go straight to connection test
        println!(
            "{}{} {}",
            prefix,
            colorize("✓", SUCCESS),
            colorize("All dependencies satisfied.", BOLD),
        );
        println!("{}", prefix);
        return test_and_save_connection(state).await;
    }

    // ── Show multi-select checklist ─────────────────────────────────────
    let options: Vec<SelectOption<'_>> = checklist
        .iter()
        .map(|item| SelectOption {
            label: item.label,
            description: item.description,
        })
        .collect();

    // Pre-check all items by default
    let defaults: Vec<bool> = vec![true; options.len()];

    let selected =
        prompt_multi_select_with_defaults("Select actions to perform", &options, &defaults)?;

    if selected.is_empty() {
        println!("{}{}", prefix, colorize("No actions selected.", DIM),);
        println!("{}", prefix);

        let manual = prompt_yes_no("Enter a database URL manually?", true)?;
        if manual {
            return manual_url_entry(state).await;
        }
        state.config.database_url = None;
        return Ok(StepResult::Next);
    }

    println!("{}", prefix);

    // ── Execute selected actions sequentially ────────────────────────────
    let selected_actions: Vec<Action> = selected.iter().map(|&i| checklist[i].action).collect();
    let mut any_failed = false;

    for &action in &selected_actions {
        if execute_action(action, &paths, &prefix).is_err() {
            any_failed = true;
            // Continue with remaining actions — some may still work
        }

        // Re-discover paths after install (new binaries may be available)
        if action == Action::InstallPostgres {
            paths = discover_pg_paths();
        }
    }

    // If pgvector was just installed but enable wasn't in the list, try enabling
    if selected_actions.contains(&Action::InstallPgvector)
        && !selected_actions.contains(&Action::EnablePgvector)
    {
        // Re-detect after install
        let fresh_status = detect_status(&paths);
        if fresh_status.pgvector_pkg_installed && !fresh_status.pgvector_ext_enabled {
            let _ = execute_action(Action::EnablePgvector, &paths, &prefix);
        }
    }

    println!("{}", prefix);

    if any_failed {
        println!(
            "{}{} Some actions failed. You can still try connecting.",
            prefix,
            colorize("!", WARNING),
        );
        println!("{}", prefix);
    }

    // ── Connection test ─────────────────────────────────────────────────
    test_and_save_connection(state).await
}

/// Print a compact status overview of all dependencies.
fn print_status_overview(prefix: &str, status: &DepStatus) {
    let check = |ok: bool, label: &str| {
        if ok {
            println!("{}{} {}", prefix, colorize("✓", SUCCESS), label);
        } else {
            println!("{}{} {}", prefix, colorize("✗", ERROR), label);
        }
    };

    check(status.pg_installed, "PostgreSQL installed");
    check(status.pg_running, "PostgreSQL running");
    check(
        status.db_exists,
        &format!("Database '{}' exists", DEFAULT_DB_NAME),
    );
    check(
        status.pgvector_pkg_installed,
        "pgvector available (optional)",
    );
    println!("{}", prefix);
}

/// Test connection and save URL to config.
async fn test_and_save_connection(state: &mut WizardState) -> Result<StepResult> {
    let prefix = step_prefix();
    let url = DEFAULT_DATABASE_URL.to_string();

    let mut spinner = Spinner::new("Testing connection and running migrations...");
    spinner.start();
    let result = storage::StoragePool::connect(&url).await;
    spinner.stop();

    match result {
        Ok(_pool) => {
            println!(
                "{}{} {}",
                prefix,
                colorize("✓", SUCCESS),
                colorize("Connected! Migrations applied successfully.", BOLD),
            );
            state.config.database_url = Some(url);
        }
        Err(e) => {
            println!(
                "{}{} {} {}",
                prefix,
                colorize("✗", ERROR),
                colorize("Connection failed:", ERROR),
                e,
            );
            println!("{}", prefix);

            let manual = prompt_yes_no("Enter a different database URL?", true)?;
            if manual {
                return manual_url_entry(state).await;
            }

            let save_anyway = prompt_yes_no("Save default URL anyway (fix later)?", false)?;
            if save_anyway {
                state.config.database_url = Some(url);
            } else {
                state.config.database_url = None;
            }
        }
    }

    Ok(StepResult::Next)
}

/// Fallback: let the user type a custom database URL, test it, and save.
async fn manual_url_entry(state: &mut WizardState) -> Result<StepResult> {
    let prefix = step_prefix();

    let current_url = state
        .config
        .database_url
        .as_deref()
        .unwrap_or(DEFAULT_DATABASE_URL);
    let url = prompt_text("Database URL", Some(current_url), true)?;

    let test = prompt_yes_no("Test connection now?", true)?;

    if test {
        let mut spinner = Spinner::new("Connecting...");
        spinner.start();
        let result = storage::StoragePool::connect(&url).await;
        spinner.stop();

        match result {
            Ok(_pool) => {
                println!(
                    "{}{} {}",
                    prefix,
                    colorize("✓", SUCCESS),
                    colorize("Connected! Migrations applied.", BOLD),
                );
                state.config.database_url = Some(url);
            }
            Err(e) => {
                println!(
                    "{}{} {} {}",
                    prefix,
                    colorize("✗", ERROR),
                    colorize("Connection failed:", ERROR),
                    e,
                );

                let save = prompt_yes_no("Save URL anyway (fix later)?", false)?;
                if save {
                    state.config.database_url = Some(url);
                } else {
                    state.config.database_url = None;
                }
            }
        }
    } else {
        state.config.database_url = Some(url);
        println!(
            "{}{}",
            prefix,
            colorize("URL saved. Test later with `klyntbot init`.", DIM),
        );
    }

    Ok(StepResult::Next)
}

// ============================================================================
// Helpers
// ============================================================================

/// Build the `│ ` prefix for consistent vertical line styling.
fn step_prefix() -> String {
    let chars = BoxChars::get();
    format!("{} ", colorize(chars.vertical, BRAND))
}

/// Mask a database URL for display, showing only the host portion.
///
/// `postgresql://user:pass@host:5432/db` -> `postgresql://***@host:5432/db`
fn mask_url(url: &str) -> String {
    if let Some(at_pos) = url.find('@') {
        if let Some(scheme_end) = url.find("://") {
            let scheme = &url[..scheme_end + 3];
            let after_at = &url[at_pos..];
            return format!("{}***{}", scheme, after_at);
        }
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_url_with_credentials() {
        assert_eq!(
            mask_url("postgresql://user:pass@localhost:5432/klyntbot"),
            "postgresql://***@localhost:5432/klyntbot"
        );
    }

    #[test]
    fn test_mask_url_without_credentials() {
        assert_eq!(
            mask_url("postgresql://localhost/klyntbot"),
            "postgresql://localhost/klyntbot"
        );
    }

    #[test]
    fn test_mask_url_user_only() {
        assert_eq!(
            mask_url("postgresql://admin@localhost/klyntbot"),
            "postgresql://***@localhost/klyntbot"
        );
    }

    #[test]
    fn test_default_database_url() {
        assert_eq!(DEFAULT_DATABASE_URL, "postgresql://localhost/klyntbot");
    }

    #[test]
    fn test_find_pg_binary_nonexistent() {
        assert!(find_pg_binary("definitely_not_a_real_pg_binary_xyz").is_none());
    }

    #[test]
    fn test_scan_homebrew_cellar_nonexistent_base() {
        assert!(scan_homebrew_cellar("/nonexistent/path", "psql").is_none());
    }

    #[test]
    fn test_scan_linux_pg_dirs_nonexistent() {
        // On macOS this path doesn't exist, so should return None gracefully
        if !std::path::Path::new("/usr/lib/postgresql").exists() {
            assert!(scan_linux_pg_dirs("psql").is_none());
        }
    }

    #[test]
    fn test_build_checklist_all_missing() {
        let status = DepStatus {
            has_homebrew: true,
            has_apt: false,
            pg_installed: false,
            pg_running: false,
            db_exists: false,
            pgvector_pkg_installed: false,
            pgvector_ext_enabled: false,
        };
        let items = build_checklist(&status);
        // Should have: install PG, create DB, install pgvector
        assert!(items.iter().any(|i| i.action == Action::InstallPostgres));
        assert!(items.iter().any(|i| i.action == Action::CreateDatabase));
        assert!(items.iter().any(|i| i.action == Action::InstallPgvector));
    }

    #[test]
    fn test_build_checklist_pg_installed_not_running() {
        let status = DepStatus {
            has_homebrew: true,
            has_apt: false,
            pg_installed: true,
            pg_running: false,
            db_exists: false,
            pgvector_pkg_installed: false,
            pgvector_ext_enabled: false,
        };
        let items = build_checklist(&status);
        assert!(items.iter().any(|i| i.action == Action::StartPostgres));
        assert!(!items.iter().any(|i| i.action == Action::InstallPostgres));
    }

    #[test]
    fn test_build_checklist_all_satisfied() {
        let status = DepStatus {
            has_homebrew: true,
            has_apt: false,
            pg_installed: true,
            pg_running: true,
            db_exists: true,
            pgvector_pkg_installed: true,
            pgvector_ext_enabled: true,
        };
        let items = build_checklist(&status);
        assert!(items.is_empty());
    }

    #[test]
    fn test_build_checklist_pgvector_needs_enable() {
        let status = DepStatus {
            has_homebrew: true,
            has_apt: false,
            pg_installed: true,
            pg_running: true,
            db_exists: true,
            pgvector_pkg_installed: true,
            pgvector_ext_enabled: false,
        };
        let items = build_checklist(&status);
        assert!(items.iter().any(|i| i.action == Action::EnablePgvector));
        assert!(!items.iter().any(|i| i.action == Action::InstallPgvector));
    }

    #[test]
    fn test_pg_paths_is_complete() {
        let complete = PgPaths {
            psql: Some(PathBuf::from("/usr/bin/psql")),
            pg_isready: Some(PathBuf::from("/usr/bin/pg_isready")),
            createdb: Some(PathBuf::from("/usr/bin/createdb")),
        };
        assert!(complete.is_complete());

        let incomplete = PgPaths {
            psql: Some(PathBuf::from("/usr/bin/psql")),
            pg_isready: None,
            createdb: Some(PathBuf::from("/usr/bin/createdb")),
        };
        assert!(!incomplete.is_complete());
    }
}
