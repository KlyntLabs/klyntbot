//! CLI command definitions using clap.

use clap::{Parser, Subcommand};

/// klyntbot - Personal AI Assistant
#[derive(Parser, Debug)]
#[command(name = "klyntbot")]
#[command(
    about = "🐈 klyntbot - Personal AI Assistant",
    long_about = "A versatile AI assistant that connects to multiple chat platforms\nand provides agent-driven automation with skills, cron jobs, and more."
)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Chat with the AI assistant (interactive or single message)
    Chat {
        /// Message to send (omit for interactive mode)
        message: Option<String>,

        /// Session ID for conversation continuity
        #[arg(short, long, default_value = "cli:default")]
        session: String,
    },

    /// Start the gateway daemon to enable channel integrations
    Serve {
        /// Port for the gateway service
        #[arg(short, long, default_value = "18790")]
        port: u16,

        /// Enable verbose debug logging
        #[arg(short, long)]
        verbose: bool,
    },

    /// Initialize klyntbot configuration and workspace
    Init,

    /// Display system status and configuration
    Status {
        /// Show detailed status including channel states
        #[arg(short, long)]
        verbose: bool,
    },

    /// Manage chat platform channels (Telegram, Discord, Slack, etc.)
    #[command(subcommand)]
    Channels(ChannelCommands),

    /// Schedule and manage automated jobs
    #[command(subcommand)]
    Cron(CronCommands),

    /// View and modify configuration settings
    #[command(subcommand)]
    Config(ConfigCommands),

    /// Manage and view available skills
    #[command(subcommand)]
    Skills(SkillsCommands),

    /// Manage todo tasks (add, list, focus, complete)
    #[command(subcommand)]
    Todo(TodoCommands),

    /// Manage projects (create, list, show, update, archive)
    #[command(subcommand)]
    Project(ProjectCommands),

    /// Calendar operations (sync, reconcile, status)
    #[command(subcommand)]
    Calendar(CalendarCommands),
}

#[derive(Subcommand, Debug)]
pub enum ChannelCommands {
    /// List all available channels and their configuration status
    List,

    /// Show setup instructions for a specific channel
    Login {
        /// Channel name: telegram, discord, whatsapp, slack, email, qq
        channel: String,
    },

    /// Test if a channel is properly configured and can connect
    Test {
        /// Channel name to test
        channel: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum CronCommands {
    /// List scheduled jobs
    List {
        /// Include disabled jobs
        #[arg(short, long)]
        all: bool,
    },

    /// Add a new scheduled job
    Add {
        /// Job name
        #[arg(short, long)]
        name: String,

        /// Message for the agent
        #[arg(short, long)]
        message: String,

        /// Run every N seconds
        #[arg(short, long)]
        every: Option<u64>,

        /// Cron expression (e.g., "0 9 * * *")
        #[arg(short, long)]
        cron: Option<String>,

        /// Run once at specified time (ISO format)
        #[arg(short, long)]
        at: Option<String>,

        /// Deliver response to channel
        #[arg(short, long)]
        deliver: bool,

        /// Recipient for delivery
        #[arg(short, long)]
        to: Option<String>,

        /// Channel for delivery
        #[arg(long)]
        channel: Option<String>,
    },

    /// Remove a scheduled job
    Remove {
        /// Job ID
        job_id: String,
    },

    /// Manually run a job
    Run {
        /// Job ID
        job_id: String,

        /// Force run even if disabled
        #[arg(short, long)]
        force: bool,
    },

    /// Enable a job
    Enable {
        /// Job ID
        job_id: String,
    },

    /// Disable a job
    Disable {
        /// Job ID
        job_id: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Show current configuration
    Show,

    /// Get a configuration value
    Get {
        /// Configuration key (e.g., "agents.defaults.model")
        key: String,
    },

    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,

        /// Value to set
        value: String,
    },

    /// Edit configuration in $EDITOR
    Edit,

    /// Reset to default configuration
    Reset {
        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Validate configuration file
    Validate,
}

#[derive(Subcommand, Debug)]
pub enum SkillsCommands {
    /// List all available skills with their status
    List,

    /// Display detailed information about a specific skill
    Info {
        /// Name of the skill to inspect
        name: String,
    },

    /// Show the filesystem path to the skills directory
    Path,
}

#[derive(Subcommand, Debug)]
pub enum TodoCommands {
    /// Add a new task
    Add {
        /// Task title
        title: String,

        /// Task description
        #[arg(short, long)]
        description: Option<String>,

        /// Priority (1-5, where 5 is critical)
        #[arg(short, long)]
        priority: Option<u8>,

        /// Due date (YYYY-MM-DD)
        #[arg(short = 'D', long)]
        due: Option<String>,

        /// Tags (comma-separated)
        #[arg(short, long)]
        tags: Option<String>,
    },

    /// List tasks
    List {
        /// Filter by status (todo, doing, done, archived)
        #[arg(short, long)]
        status: Option<String>,

        /// Filter by tag
        #[arg(short, long)]
        tag: Option<String>,

        /// Minimum priority (1-5)
        #[arg(short = 'P', long)]
        priority_min: Option<u8>,

        /// Limit number of results
        #[arg(short, long)]
        limit: Option<usize>,
    },

    /// Show task details
    Show {
        /// Task ID
        id: String,
    },

    /// Mark task as complete
    Complete {
        /// Task ID
        id: String,
    },

    /// Delete a task
    Delete {
        /// Task ID
        id: String,
    },

    /// Focus on a task (adds to focus board)
    Focus {
        /// Task ID (omit to show focus board)
        id: Option<String>,
    },

    /// Unfocus a task (removes from focus board)
    Unfocus {
        /// Task ID
        id: String,
    },

    /// Show task summary and statistics
    Summary,

    /// View tasks as a hierarchical tree
    Tree {
        /// Filter by project ID
        #[arg(short, long)]
        project: Option<String>,

        /// Maximum nesting depth
        #[arg(short, long)]
        depth: Option<usize>,
    },

    /// Search tasks by keyword
    Search {
        /// Search query
        query: String,

        /// Also search attachment content
        #[arg(short, long)]
        include_attachments: bool,
    },

    /// Update task fields
    Update {
        /// Task ID
        id: String,

        /// New title
        #[arg(short, long)]
        title: Option<String>,

        /// New description
        #[arg(long)]
        description: Option<String>,

        /// New priority (1-5)
        #[arg(short, long)]
        priority: Option<u8>,

        /// New due date (supports natural language: "tomorrow", "next Friday")
        #[arg(short = 'D', long)]
        due: Option<String>,

        /// New tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,

        /// New status (todo, doing, done, archived)
        #[arg(short, long)]
        status: Option<String>,
    },

    /// Attach a file, URL, or note to a task
    Attach {
        /// Task ID
        id: String,

        /// Attach a file by path
        #[arg(short, long)]
        file: Option<String>,

        /// Attach a URL
        #[arg(short, long)]
        url: Option<String>,

        /// Attach a text note
        #[arg(short, long)]
        note: Option<String>,

        /// Attachment title
        #[arg(short, long)]
        title: Option<String>,
    },

    /// Remove an attachment from a task
    Detach {
        /// Task ID
        id: String,

        /// Attachment ID to remove
        attachment_id: String,
    },

    /// Add a subtask to an existing task
    AddSubtask {
        /// Parent task ID
        parent_id: String,

        /// Subtask title
        title: String,

        /// Subtask description
        #[arg(short, long)]
        description: Option<String>,

        /// Priority (1-5)
        #[arg(short, long)]
        priority: Option<u8>,

        /// Due date
        #[arg(short = 'D', long)]
        due: Option<String>,

        /// Tags (comma-separated)
        #[arg(short, long)]
        tags: Option<String>,
    },

    /// Move a task to a different parent or project
    Move {
        /// Task ID
        id: String,

        /// New parent task ID (use "none" to make top-level)
        #[arg(short = 'P', long)]
        parent: Option<String>,

        /// New project ID (use "none" to remove from project)
        #[arg(long)]
        project: Option<String>,
    },

    /// Log time spent on a task
    LogTime {
        /// Task ID
        id: String,

        /// Minutes spent
        minutes: u32,

        /// Optional note about the work done
        #[arg(short, long)]
        note: Option<String>,
    },

    /// Generate a time tracking and completion report
    Report {
        /// Report period
        #[arg(short = 'P', long, default_value = "week")]
        period: String,

        /// Filter by project ID
        #[arg(long)]
        project: Option<String>,
    },

    /// Manage task dependencies
    Depend {
        /// Task ID
        id: String,

        /// Add dependency: this task is blocked by the given task ID
        #[arg(long)]
        on: Option<String>,

        /// Remove dependency: this task is no longer blocked by the given task ID
        #[arg(long)]
        remove: Option<String>,
    },

    /// Manage recurring task templates
    #[command(subcommand)]
    Recur(RecurCommands),
}

#[derive(Subcommand, Debug)]
pub enum RecurCommands {
    /// Create a new recurring task template
    Add {
        /// Task title
        title: String,

        /// RRULE recurrence string, e.g. "FREQ=DAILY;BYHOUR=9"
        #[arg(short, long)]
        rule: String,

        /// Task description
        #[arg(short, long)]
        description: Option<String>,

        /// Priority (1-5)
        #[arg(short, long)]
        priority: Option<u8>,

        /// Tags (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        tags: Option<Vec<String>>,

        /// Project ID to associate with
        #[arg(long)]
        project: Option<String>,
    },

    /// List all recurring task templates
    List {
        /// Filter by project ID
        #[arg(long)]
        project: Option<String>,
    },

    /// Delete a recurring task template
    Delete {
        /// Template ID to delete
        id: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ProjectCommands {
    /// Create a new project
    Create {
        /// Project name
        name: String,

        /// Project description
        #[arg(short, long)]
        description: Option<String>,

        /// Project color (red, orange, yellow, green, blue, purple, gray)
        #[arg(short, long)]
        color: Option<String>,

        /// Tags (comma-separated)
        #[arg(short, long)]
        tags: Option<String>,
    },

    /// List projects
    List {
        /// Filter by status (active, paused, completed, archived)
        #[arg(short, long)]
        status: Option<String>,

        /// Filter by tag
        #[arg(short, long)]
        tag: Option<String>,

        /// Limit number of results
        #[arg(short, long)]
        limit: Option<usize>,
    },

    /// Show project details
    Show {
        /// Project ID
        id: String,
    },

    /// Update project fields
    Update {
        /// Project ID
        id: String,

        /// New name
        #[arg(short, long)]
        name: Option<String>,

        /// New description
        #[arg(short, long)]
        description: Option<String>,

        /// New color
        #[arg(short, long)]
        color: Option<String>,

        /// New status (active, paused, completed, archived)
        #[arg(short, long)]
        status: Option<String>,

        /// New tags (comma-separated)
        #[arg(short, long)]
        tags: Option<String>,
    },

    /// Archive a project
    Archive {
        /// Project ID
        id: String,
    },

    /// List tasks belonging to a project
    Tasks {
        /// Project ID
        id: String,

        /// Limit number of results
        #[arg(short, long)]
        limit: Option<usize>,

        /// Show as tree instead of flat list
        #[arg(short, long)]
        tree: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum CalendarCommands {
    /// Manually trigger calendar reconciliation (check event status and update linked todos)
    Reconcile,
}
