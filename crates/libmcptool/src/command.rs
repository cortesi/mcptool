use clap::{Args, CommandFactory, Parser, Subcommand};
use tmcp::{Client, ClientHandler, schema::InitializeResult};

use crate::{Result, client, ctx::Ctx, mcp, target::Target};

#[derive(Args)]
pub struct McpArgs {
    // No longer needed - auth is now handled via auth:// target syntax
}

// Base commands without target - used by both CLI and REPL
#[derive(Subcommand)]
#[command(no_binary_name = true)]
pub enum McpCommand {
    /// Send a ping request to an MCP server
    Ping,

    /// List all MCP tools from a server
    Listtools,

    /// Initialize connection and display server information
    Init,

    /// List all MCP resources from a server
    Listresources,

    /// List all MCP prompts from a server
    Listprompts,

    /// List all MCP resource templates from a server
    Listresourcetemplates,

    /// Set the logging level on the MCP server
    Setlevel {
        /// The logging level to set (debug, info, notice, warning, error, critical, alert, emergency)
        level: String,
    },

    /// Call an MCP tool with various input modes
    Calltool {
        /// Name of the tool to call
        tool_name: String,

        /// Arguments in key=value format (can be specified multiple times)
        #[arg(long = "arg", short = 'a')]
        args: Vec<String>,

        /// Interactive mode: prompt for each tool parameter
        #[arg(long, short)]
        interactive: bool,

        /// JSON mode: read arguments from stdin as JSON
        #[arg(long, short)]
        json: bool,
    },

    /// Read a resource by URI
    Readresource {
        /// URI of the resource to read
        uri: String,
    },

    /// Get a prompt by name with optional arguments
    Getprompt {
        /// Name of the prompt to get
        name: String,

        /// Arguments in key=value format (can be specified multiple times)
        #[arg(long = "arg", short = 'a')]
        args: Vec<String>,
    },

    /// Subscribe to resource update notifications
    Subscriberesource {
        /// URI of the resource to subscribe to
        uri: String,
    },

    /// Unsubscribe from resource update notifications
    Unsubscriberesource {
        /// URI of the resource to unsubscribe from
        uri: String,
    },

    /// Get completion suggestions for prompt or resource arguments
    Complete {
        /// Reference to complete (e.g., "resource://uri" or "prompt://name")
        reference: String,

        /// Name of the argument to complete
        argument: String,
    },
}

// For CLI use - target is required at this level
#[derive(Parser)]
pub struct CliMcpCommand {
    /// The MCP server target (e.g., "localhost:3000", "tcp://host:port", "http://host:port", "auth://name")
    pub target: String,

    #[command(subcommand)]
    pub command: McpCommand,
}

// For REPL use - no target needed, just the command
#[derive(Parser)]
#[command(no_binary_name = true)]
pub struct ReplCommandWrapper {
    #[command(subcommand)]
    pub command: McpCommand,
}

// For REPL use - reuses existing client connection
pub async fn execute_mcp_command_with_client<C: ClientHandler + 'static>(
    command: McpCommand,
    client: &mut Client<C>,
    init_result: &InitializeResult,
    ctx: &Ctx,
) -> Result<()> {
    match command {
        McpCommand::Ping => {
            mcp::ping(client, &ctx.output).await?;
        }
        McpCommand::Listtools => {
            mcp::listtools(client, &ctx.output).await?;
        }
        McpCommand::Init => {
            mcp::init(init_result, &ctx.output)?;
        }
        McpCommand::Listresources => {
            mcp::listresources(client, &ctx.output).await?;
        }
        McpCommand::Listprompts => {
            mcp::listprompts(client, &ctx.output).await?;
        }
        McpCommand::Listresourcetemplates => {
            mcp::listresourcetemplates(client, &ctx.output).await?;
        }
        McpCommand::Setlevel { level } => {
            mcp::set_level(client, &ctx.output, &level).await?;
        }
        McpCommand::Calltool {
            tool_name,
            args,
            interactive,
            json,
        } => {
            mcp::calltool(client, &ctx.output, &tool_name, args, interactive, json).await?;
        }
        McpCommand::Readresource { uri } => {
            mcp::read_resource(client, &ctx.output, &uri).await?;
        }
        McpCommand::Getprompt { name, args } => {
            mcp::get_prompt(client, &ctx.output, &name, args).await?;
        }
        McpCommand::Subscriberesource { uri } => {
            mcp::subscribe_resource(client, &ctx.output, &uri).await?;
        }
        McpCommand::Unsubscriberesource { uri } => {
            mcp::unsubscribe_resource(client, &ctx.output, &uri).await?;
        }
        McpCommand::Complete {
            reference,
            argument,
        } => {
            mcp::complete(client, &ctx.output, &reference, &argument).await?;
        }
    }
    Ok(())
}

// For CLI use - creates new client connection for single command
pub async fn execute_mcp_command(command: McpCommand, target: &str, ctx: &Ctx) -> Result<()> {
    let target = Target::parse(target)?;
    let (mut client, init_result) = client::get_client(ctx, &target).await?;
    execute_mcp_command_with_client(command, &mut client, &init_result, ctx).await
}

/// Generate help text for the REPL using clap's built-in help generation
pub fn generate_repl_help() -> String {
    let mut help = String::new();
    help.push_str("Available MCP commands:\n\n");

    // Create a temporary app to get the help for the subcommand
    let wrapper_cmd = ReplCommandWrapper::command();

    // The ReplCommandWrapper directly contains the McpCommand subcommands
    for cmd in wrapper_cmd.get_subcommands() {
        let name = cmd.get_name();
        let about = cmd
            .get_about()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "".to_string());
        help.push_str(&format!("  {:20} - {}\n", name, about));
    }

    help.push_str("\nAdditional REPL commands:\n");
    help.push_str("  help                 - Show this help message\n");
    help.push_str("  quit/exit            - Exit the REPL\n");

    help
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_repl_help() {
        let help = generate_repl_help();

        // Check that the help contains expected sections
        assert!(help.contains("Available MCP commands:"));
        assert!(help.contains("Additional REPL commands:"));
        assert!(help.contains("help"));
        assert!(help.contains("quit/exit"));

        // Check that some of the MCP commands are included
        assert!(help.contains("ping"));
        assert!(help.contains("listtools"));
        assert!(help.contains("init"));

        // Check that the help is not empty
        assert!(!help.is_empty());
    }
}
