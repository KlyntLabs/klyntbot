//! Database configuration wizard step.
//!
//! Guides users through installing, starting, and configuring PostgreSQL
//! for klyntbot's persistent storage layer. Detects the current system state
//! and walks through each required step:
//!
//! 1. Check if PostgreSQL is installed (look for `pg_isready` / `psql`)
//! 2. Check if PostgreSQL is running
//! 3. Create the `klyntbot` database if missing
//! 4. Check / install pgvector extension
//! 5. Test connection + run migrations
//! 6. Save `database_url` to config

use std::process::Command;

use anyhow::Result;
use common::utils::terminal::*;

use crate::wizard::framework::{StepResult, WizardState};
use crate::wizard::prompts::{prompt_text, prompt_yes_no};

/// Default PostgreSQL connection URL.
const DEFAULT_DATABASE_URL: &str = "postgresql://localhost/klyntbot";

/// Default database name to create.
const DEFAULT_DB_NAME: &str = "klyntbot";

// ============================================================================
// Detection helpers
// ============================================================================

/// Check if a command exists on PATH by running it with a harmless flag.
fn command_exists(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if PostgreSQL is accepting connections via `pg_isready`.
fn pg_is_running() -> bool {
    Command::new("pg_isready")
        .arg("--quiet")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if a database exists by querying `psql`.
fn database_exists(db_name: &str) -> bool {
    Command::new("psql")
        .args(["-d", db_name, "-c", "SELECT 1"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Try to create a database using `createdb`.
fn create_database(db_name: &str) -> std::result::Result<(), String> {
    let output = Command::new("createdb")
        .arg(db_name)
        .output()
        .map_err(|e| format!("Failed to run createdb: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(stderr.trim().to_string())
    }
}

/// Check if pgvector extension is available in PostgreSQL.
fn pgvector_available(db_name: &str) -> bool {
    Command::new("psql")
        .args([
            "-d",
            db_name,
            "-t",
            "-c",
            "SELECT 1 FROM pg_available_extensions WHERE name = 'vector'",
        ])
        .output()
        .map(|o| {
            o.status.success()
                && String::from_utf8_lossy(&o.stdout).trim().contains('1')
        })
        .unwrap_or(false)
}

/// Check if pgvector extension is already installed (CREATE EXTENSION done).
fn pgvector_installed(db_name: &str) -> bool {
    Command::new("psql")
        .args([
            "-d",
            db_name,
            "-t",
            "-c",
            "SELECT 1 FROM pg_extension WHERE extname = 'vector'",
        ])
        .output()
        .map(|o| {
            o.status.success()
                && String::from_utf8_lossy(&o.stdout).trim().contains('1')
        })
        .unwrap_or(false)
}

/// Install pgvector extension in the database.
fn install_pgvector(db_name: &str) -> std::result::Result<(), String> {
    let output = Command::new("psql")
        .args([
            "-d",
            db_name,
            "-c",
            "CREATE EXTENSION IF NOT EXISTS vector",
        ])
        .output()
        .map_err(|e| format!("Failed to run psql: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(stderr.trim().to_string())
    }
}

/// Detect the platform and return appropriate install instructions.
fn install_instructions() -> (&'static str, &'static str) {
    if cfg!(target_os = "macos") {
        (
            "brew install postgresql@17",
            "brew services start postgresql@17",
        )
    } else {
        (
            "sudo apt install postgresql postgresql-contrib",
            "sudo systemctl start postgresql",
        )
    }
}

/// Return pgvector install instructions for the current platform.
fn pgvector_install_instructions() -> &'static str {
    if cfg!(target_os = "macos") {
        "brew install pgvector"
    } else {
        "sudo apt install postgresql-17-pgvector"
    }
}

// ============================================================================
// Main wizard step
// ============================================================================

/// Run the guided database configuration step.
///
/// Detects PostgreSQL state on the system and walks users through
/// installation, startup, database creation, pgvector setup, and
/// connection testing — all step by step.
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
    println!(
        "{}{}",
        prefix,
        colorize("Let's make sure PostgreSQL is set up on your system.", DIM)
    );
    println!("{}", prefix);

    // ── Step 1: Check PostgreSQL installation ───────────────────────────
    let pg_installed = command_exists("psql") && command_exists("pg_isready");

    if !pg_installed {
        let (install_cmd, start_cmd) = install_instructions();
        println!(
            "{}{} {}",
            prefix,
            colorize("!", WARNING),
            colorize("PostgreSQL is not installed.", BOLD),
        );
        println!("{}", prefix);
        println!(
            "{}  Install it with:",
            prefix,
        );
        println!(
            "{}  {} {}",
            prefix,
            colorize("$", DIM),
            colorize(install_cmd, HIGHLIGHT),
        );
        println!("{}", prefix);
        println!(
            "{}  Then start the service:",
            prefix,
        );
        println!(
            "{}  {} {}",
            prefix,
            colorize("$", DIM),
            colorize(start_cmd, HIGHLIGHT),
        );
        println!("{}", prefix);

        let proceed_anyway = prompt_yes_no(
            "Continue with manual URL entry? (if PostgreSQL is elsewhere)",
            false,
        )?;

        if proceed_anyway {
            return manual_url_entry(state).await;
        }

        println!(
            "{}{}",
            prefix,
            colorize(
                "Install PostgreSQL and re-run `klyntbot init` to continue setup.",
                DIM,
            )
        );
        state.config.database_url = None;
        return Ok(StepResult::Next);
    }

    println!(
        "{}{} {}",
        prefix,
        colorize("✓", SUCCESS),
        "PostgreSQL is installed.",
    );

    // ── Step 2: Check PostgreSQL is running ─────────────────────────────
    if !pg_is_running() {
        let (_, start_cmd) = install_instructions();
        println!(
            "{}{} {}",
            prefix,
            colorize("!", WARNING),
            colorize("PostgreSQL is not running.", BOLD),
        );
        println!("{}", prefix);
        println!(
            "{}  Start it with:",
            prefix,
        );
        println!(
            "{}  {} {}",
            prefix,
            colorize("$", DIM),
            colorize(start_cmd, HIGHLIGHT),
        );
        println!("{}", prefix);

        let wait = prompt_yes_no("I've started PostgreSQL — check again?", true)?;

        if wait && pg_is_running() {
            println!(
                "{}{} {}",
                prefix,
                colorize("✓", SUCCESS),
                "PostgreSQL is running.",
            );
        } else if wait {
            println!(
                "{}{} {}",
                prefix,
                colorize("✗", ERROR),
                "Still not responding. Start PostgreSQL and re-run `klyntbot init`.",
            );
            state.config.database_url = None;
            return Ok(StepResult::Next);
        } else {
            let proceed_anyway = prompt_yes_no(
                "Continue with manual URL entry?",
                false,
            )?;

            if proceed_anyway {
                return manual_url_entry(state).await;
            }

            state.config.database_url = None;
            return Ok(StepResult::Next);
        }
    } else {
        println!(
            "{}{} {}",
            prefix,
            colorize("✓", SUCCESS),
            "PostgreSQL is running.",
        );
    }

    // ── Step 3: Create database ─────────────────────────────────────────
    if database_exists(DEFAULT_DB_NAME) {
        println!(
            "{}{} Database `{}` exists.",
            prefix,
            colorize("✓", SUCCESS),
            DEFAULT_DB_NAME,
        );
    } else {
        println!(
            "{}{}  Database `{}` does not exist yet.",
            prefix,
            colorize("·", DIM),
            DEFAULT_DB_NAME,
        );
        let create = prompt_yes_no(
            &format!("Create database `{}`?", DEFAULT_DB_NAME),
            true,
        )?;

        if create {
            match create_database(DEFAULT_DB_NAME) {
                Ok(()) => {
                    println!(
                        "{}{} Database `{}` created.",
                        prefix,
                        colorize("✓", SUCCESS),
                        DEFAULT_DB_NAME,
                    );
                }
                Err(e) => {
                    println!(
                        "{}{} Failed to create database: {}",
                        prefix,
                        colorize("✗", ERROR),
                        e,
                    );
                    println!(
                        "{}  Create it manually: {} {}",
                        prefix,
                        colorize("$", DIM),
                        colorize(&format!("createdb {}", DEFAULT_DB_NAME), HIGHLIGHT),
                    );
                }
            }
        }
    }

    // ── Step 4: pgvector extension ──────────────────────────────────────
    if database_exists(DEFAULT_DB_NAME) {
        if pgvector_installed(DEFAULT_DB_NAME) {
            println!(
                "{}{} pgvector extension is installed.",
                prefix,
                colorize("✓", SUCCESS),
            );
        } else if pgvector_available(DEFAULT_DB_NAME) {
            println!(
                "{}{}  pgvector is available but not yet enabled.",
                prefix,
                colorize("·", DIM),
            );
            let enable = prompt_yes_no("Enable pgvector extension?", true)?;

            if enable {
                match install_pgvector(DEFAULT_DB_NAME) {
                    Ok(()) => {
                        println!(
                            "{}{} pgvector extension enabled.",
                            prefix,
                            colorize("✓", SUCCESS),
                        );
                    }
                    Err(e) => {
                        println!(
                            "{}{} Failed to enable pgvector: {}",
                            prefix,
                            colorize("✗", ERROR),
                            e,
                        );
                        println!(
                            "{}  {}",
                            prefix,
                            colorize(
                                "Semantic search will be unavailable without pgvector.",
                                DIM,
                            ),
                        );
                    }
                }
            }
        } else {
            let install_cmd = pgvector_install_instructions();
            println!(
                "{}{} pgvector extension is not installed on your system.",
                prefix,
                colorize("!", WARNING),
            );
            println!("{}", prefix);
            println!(
                "{}  Install it with:",
                prefix,
            );
            println!(
                "{}  {} {}",
                prefix,
                colorize("$", DIM),
                colorize(install_cmd, HIGHLIGHT),
            );
            println!("{}", prefix);
            println!(
                "{}  {}",
                prefix,
                colorize(
                    "Semantic search requires pgvector. klyntbot will work without it,",
                    DIM,
                ),
            );
            println!(
                "{}  {}",
                prefix,
                colorize(
                    "but vector similarity search won't be available.",
                    DIM,
                ),
            );
        }
    }

    // ── Step 5: Test connection + run migrations ────────────────────────
    let url = DEFAULT_DATABASE_URL.to_string();

    println!("{}", prefix);
    println!(
        "{}{}",
        prefix,
        colorize("Testing connection and running migrations...", DIM),
    );

    match storage::StoragePool::connect(&url).await {
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
        println!(
            "{}{}",
            prefix,
            colorize("Connecting...", DIM),
        );

        match storage::StoragePool::connect(&url).await {
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
/// `postgresql://user:pass@host:5432/db` → `postgresql://***@host:5432/db`
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
    fn test_command_exists_false_for_nonexistent() {
        assert!(!command_exists("definitely_not_a_real_command_xyz"));
    }

    #[test]
    fn test_install_instructions_non_empty() {
        let (install, start) = install_instructions();
        assert!(!install.is_empty());
        assert!(!start.is_empty());
    }

    #[test]
    fn test_pgvector_install_instructions_non_empty() {
        let cmd = pgvector_install_instructions();
        assert!(!cmd.is_empty());
    }
}
