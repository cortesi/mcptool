//! mcptool CLI application for interacting with MCP servers.
#![allow(clippy::missing_docs_in_private_items)]
use std::error::Error;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use libmcptool::{
    LogLevel, auth,
    command::{CliMcpCommand, execute_mcp_command},
    connect, ctx, proxy,
    target::Target,
    testserver,
};
use terminal_size::{Width, terminal_size};
use tmcp::schema::LATEST_PROTOCOL_VERSION;

#[derive(Args)]
struct TargetArgs {
    /// The MCP server target (e.g., "localhost:3000", "tcp://host:port", "http://host:port", "https://host:port", "cmd://./server")
    target: String,
}

#[derive(Args)]
struct ProxyArgs {
    /// The MCP server target to proxy to (e.g., "localhost:3000", "tcp://host:port", "http://host:port", "https://host:port", "cmd://./server")
    target: String,

    /// File path to log all proxy traffic
    #[arg(long)]
    log_file: PathBuf,
}

#[derive(Subcommand)]
enum AuthCommands {
    /// Add a new OAuth authentication entry
    Add {
        /// Name for the authentication entry
        name: String,

        /// Server URL (e.g., https://api.example.com)
        #[arg(long)]
        server_url: Option<String>,

        /// OAuth authorization URL
        #[arg(long)]
        auth_url: Option<String>,

        /// OAuth token URL
        #[arg(long)]
        token_url: Option<String>,

        /// OAuth client ID
        #[arg(long)]
        client_id: Option<String>,

        /// OAuth client secret
        #[arg(long)]
        client_secret: Option<String>,

        /// OAuth redirect URL (if not provided, will use local server)
        #[arg(long)]
        redirect_url: Option<String>,

        /// Resource/Audience parameter for OAuth
        #[arg(long)]
        resource: Option<String>,

        /// OAuth scopes (comma-separated)
        #[arg(long)]
        scopes: Option<String>,

        /// Show the redirect URL that will be used without starting OAuth flow
        #[arg(long)]
        show_redirect_url: bool,
    },

    /// List all stored authentication entries
    #[command(alias = "ls")]
    List,

    /// Remove an authentication entry
    #[command(alias = "rm")]
    Remove {
        /// Name of the authentication entry to remove
        name: String,
    },

    /// Renew the access token for an authentication entry using the refresh token
    Renew {
        /// Name of the authentication entry to renew
        name: String,
    },
}

#[derive(Parser)]
#[command(
    name = "mcptool",
    about = "A versatile command-line utility for connecting to, testing, and probing MCP servers",
    version = ctx::VERSION,
)]
struct Cli {
    /// Output results in JSON format
    #[arg(long, global = true)]
    json: bool,

    /// Enable logging with specified level
    #[arg(long, global = true, value_enum)]
    logs: Option<LogLevel>,

    /// Force color output
    #[arg(long, global = true, conflicts_with = "no_color")]
    color: bool,

    /// Disable color output
    #[arg(long, global = true, conflicts_with = "color")]
    no_color: bool,

    /// Suppress all output including JSON output
    #[arg(long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Display the mcptool build version & linked MCP revision
    Version,

    /// MCP invocation commands (ping, listtools, etc.)
    Mcp {
        #[command(flatten)]
        mcp_command: CliMcpCommand,
    },

    /// Connect to an MCP server and start an interactive REPL
    Connect {
        /// The MCP server target (e.g., "localhost:3000", "tcp://host:port", "http://host:port", "auth://name")
        target: String,
    },

    /// Transparently proxy and print traffic forwarded to the target
    Proxy {
        #[command(flatten)]
        proxy_args: ProxyArgs,
    },

    /// Run a test MCP server with verbose logging
    Testserver {
        /// Use stdio transport instead of HTTP
        #[arg(long)]
        stdio: bool,

        /// Use TCP transport instead of HTTP
        #[arg(long)]
        tcp: bool,

        /// Port to listen on (for HTTP/TCP transport)
        #[arg(short, long, default_value = "8080")]
        port: u16,

        /// Run in interactive mode with REPL for server management
        #[arg(long)]
        interactive: bool,
    },

    /// Manage OAuth authentication entries
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    // Calculate the configuration directory
    let config_path = dirs::config_dir()
        .ok_or("Failed to get config directory")?
        .join("mcptool");

    // Determine color output preference
    let color = if cli.no_color {
        false
    } else if cli.color {
        true
    } else {
        // Auto-detect based on TTY
        atty::is(atty::Stream::Stdout)
    };

    // Detect terminal width, default to 80
    let width = if let Some((Width(w), _)) = terminal_size() {
        w as usize
    } else {
        80
    };

    // Create the MCPTool instance
    let ctx = ctx::Ctx::new(config_path, cli.logs, cli.json, cli.quiet, color, width)?;

    match cli.command {
        Commands::Version => {
            println!("mcptool version {}", ctx::VERSION);
            println!("MCP protocol version: {}", LATEST_PROTOCOL_VERSION);
        }

        Commands::Mcp { mcp_command } => {
            execute_mcp_command(mcp_command.command, &mcp_command.target, &ctx)
                .await
                .map_err(|e| Box::new(e) as Box<dyn Error>)?;
        }

        Commands::Connect { target } => {
            connect::connect_command(&ctx, target).await?;
        }

        Commands::Proxy { proxy_args } => {
            let target = Target::parse(&proxy_args.target)?;
            proxy::proxy_command(target, proxy_args.log_file).await?;
        }

        Commands::Testserver {
            stdio,
            tcp,
            port,
            interactive,
        } => {
            testserver::run_test_server(&ctx, stdio, tcp, port, interactive).await?;
        }

        Commands::Auth { command } => match command {
            AuthCommands::Add {
                name,
                server_url,
                auth_url,
                token_url,
                client_id,
                client_secret,
                redirect_url,
                resource,
                scopes,
                show_redirect_url,
            } => {
                let args = auth::AddCommandArgs {
                    name,
                    server_url,
                    auth_url,
                    token_url,
                    client_id,
                    client_secret,
                    redirect_url,
                    resource,
                    scopes,
                    show_redirect_url,
                };
                auth::add_command(&ctx, args).await?;
            }
            AuthCommands::List => auth::list_command(&ctx).await?,
            AuthCommands::Remove { name } => auth::remove_command(&ctx, name).await?,
            AuthCommands::Renew { name } => auth::renew_command(&ctx, name).await?,
        },
    }

    Ok(())
}
