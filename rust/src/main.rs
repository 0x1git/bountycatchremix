mod config;
mod db;
mod domain;
mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "bountycatch")]
#[command(about = "Ultra-fast bug bounty domain management tool", long_about = None)]
#[command(version)]
struct Cli {
    /// Configuration file path
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Suppress console logs; only emit command output
    #[arg(short, long, global = true)]
    silent: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add domains from file or stdin
    Add {
        /// File containing domains
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Skip domain validation
        #[arg(long)]
        no_validate: bool,
    },

    /// Print domains (supports filtering)
    Print {
        /// Filter domains containing this substring
        #[arg(long)]
        r#match: Option<String>,

        /// Filter domains matching this regex
        #[arg(long)]
        regex: Option<String>,

        /// Sort domains before printing
        #[arg(long)]
        sort: bool,
    },

    /// Count domains in database
    Count {
        /// Filter domains containing this substring
        #[arg(long)]
        r#match: Option<String>,

        /// Filter domains matching this regex
        #[arg(long)]
        regex: Option<String>,
    },

    /// Export domains to file
    Export {
        /// Output file
        #[arg(short, long)]
        file: PathBuf,

        /// Export format
        #[arg(long, default_value = "text")]
        format: String,

        /// Filter domains containing this substring
        #[arg(long)]
        r#match: Option<String>,

        /// Filter domains matching this regex
        #[arg(long)]
        regex: Option<String>,

        /// Sort domains before exporting
        #[arg(long)]
        sort: bool,
    },

    /// Remove domains from database
    Remove {
        /// File containing domains to remove
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Single domain to remove
        #[arg(short, long)]
        domain: Option<String>,

        /// Remove domains containing this substring
        #[arg(long)]
        r#match: Option<String>,

        /// Remove domains matching this regex
        #[arg(long)]
        regex: Option<String>,
    },

    /// Delete all domains
    DeleteAll {
        /// Skip confirmation prompt
        #[arg(long)]
        confirm: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = config::Config::load(cli.config.as_deref())?;
    
    if !cli.silent {
        if cli.verbose {
            eprintln!("Connecting to PostgreSQL at {}:{}/{}", 
                config.postgresql.host, config.postgresql.port, config.postgresql.database);
        }
    }

    let pool = db::create_pool(&config.postgresql).await?;

    if !cli.silent && cli.verbose {
        eprintln!("Connected to PostgreSQL");
    }

    // Initialize schema
    db::init_schema(&pool).await?;

    match cli.command {
        Commands::Add { file, no_validate } => {
            commands::add::run(&pool, file, !no_validate, cli.silent).await?;
        }
        Commands::Print { r#match, regex, sort } => {
            commands::print::run(&pool, r#match, regex, sort, cli.silent).await?;
        }
        Commands::Count { r#match, regex } => {
            commands::count::run(&pool, r#match, regex, cli.silent).await?;
        }
        Commands::Export { file, format, r#match, regex, sort } => {
            commands::export::run(&pool, file, format, r#match, regex, sort, cli.silent).await?;
        }
        Commands::Remove { file, domain, r#match, regex } => {
            commands::remove::run(&pool, file, domain, r#match, regex, cli.silent).await?;
        }
        Commands::DeleteAll { confirm } => {
            commands::delete_all::run(&pool, confirm, cli.silent).await?;
        }
    }

    Ok(())
}
