//! MCP client command implementations.

use tmcp::{
    Client, ClientHandler, ServerAPI,
    schema::{
        ArgumentInfo, InitializeResult, LoggingLevel, PromptReference, Reference, ResourceReference,
    },
};

use crate::{
    Error, Result, args::ArgumentParser, calltool, output, output::Output, utils::TimedFuture,
};

/// Pings the MCP server.
pub async fn ping<C: ClientHandler + 'static>(
    client: &mut Client<C>,
    output: &Output,
) -> Result<()> {
    output.text("Pinging")?;
    client.ping().timed("   response", output).await?;
    output.ping()?;
    Ok(())
}

/// Lists all available tools from the MCP server.
pub async fn listtools<C: ClientHandler + 'static>(
    client: &mut Client<C>,
    output: &Output,
) -> Result<()> {
    output.text("Listing tools")?;
    let tools_result = client
        .list_tools(None)
        .timed("    response", output)
        .await?;
    output::listtools::list_tools_result(output, &tools_result)?;
    Ok(())
}

/// Displays the initialization result from the MCP server.
pub fn init(init_result: &InitializeResult, output: &Output) -> Result<()> {
    output::initresult::init_result(output, init_result)?;
    Ok(())
}

/// Lists all available resources from the MCP server.
pub async fn listresources<C: ClientHandler + 'static>(
    client: &mut Client<C>,
    output: &Output,
) -> Result<()> {
    output.text("Listing resources")?;
    let resources_result = client
        .list_resources(None)
        .timed("    response", output)
        .await?;
    output::listresources::list_resources_result(output, &resources_result)?;
    Ok(())
}

/// Lists all available prompts from the MCP server.
pub async fn listprompts<C: ClientHandler + 'static>(
    client: &mut Client<C>,
    output: &Output,
) -> Result<()> {
    output.text("Listing prompts")?;
    let prompts_result = client
        .list_prompts(None)
        .timed("    response", output)
        .await?;
    output::listprompts::list_prompts_result(output, &prompts_result)?;
    Ok(())
}

/// Lists all available resource templates from the MCP server.
pub async fn listresourcetemplates<C: ClientHandler + 'static>(
    client: &mut Client<C>,
    output: &Output,
) -> Result<()> {
    output.text("Listing resource templates")?;
    let templates_result = client
        .list_resource_templates(None)
        .timed("    response", output)
        .await?;
    output::listresourcetemplates::list_resource_templates_result(output, &templates_result)?;
    Ok(())
}

/// Sets the logging level on the MCP server.
pub async fn set_level<C: ClientHandler + 'static>(
    client: &mut Client<C>,
    output: &Output,
    level: &str,
) -> Result<()> {
    output.text(format!("Setting logging level to: {level}"))?;

    // Parse the level string into LoggingLevel enum
    let logging_level = match level.to_lowercase().as_str() {
        "debug" => LoggingLevel::Debug,
        "info" => LoggingLevel::Info,
        "notice" => LoggingLevel::Notice,
        "warning" => LoggingLevel::Warning,
        "error" => LoggingLevel::Error,
        "critical" => LoggingLevel::Critical,
        "alert" => LoggingLevel::Alert,
        "emergency" => LoggingLevel::Emergency,
        _ => {
            return Err(Error::Other(format!(
                "Invalid logging level: {}. Valid levels are: debug, info, notice, warning, error, critical, alert, emergency",
                level
            )));
        }
    };

    // Send the set level request to the server
    client
        .set_level(logging_level)
        .timed("    response", output)
        .await?;

    output.trace_success(format!("Set logging level to: {level}"))?;
    Ok(())
}

/// Calls a tool on the MCP server.
pub async fn calltool<C: ClientHandler + 'static>(
    client: &mut Client<C>,
    output: &Output,
    tool_name: &str,
    args: Vec<String>,
    interactive: bool,
    json: bool,
) -> Result<()> {
    // Validate input modes
    let mode_count = [!args.is_empty(), interactive, json]
        .iter()
        .filter(|&&x| x)
        .count();
    if mode_count == 0 {
        return Err(Error::Other(
            "Must specify one of: --interactive, --json, or --arg key=value arguments".to_string(),
        ));
    }
    if mode_count > 1 {
        return Err(Error::Other(
            "Cannot combine --interactive, --json, and --arg modes".to_string(),
        ));
    }

    output.text(format!("Calling tool: {tool_name}"))?;

    // First, get tool schema to understand required parameters
    let tools_result = client
        .list_tools(None)
        .timed("   fetching tools", output)
        .await?;

    let tool = tools_result
        .tools
        .iter()
        .find(|t| t.name == tool_name)
        .ok_or_else(|| Error::Other(format!("Tool '{tool_name}' not found")))?;

    // Parse arguments based on mode
    let arguments = if json {
        calltool::json::parse_json_arguments(output)?
    } else if interactive {
        calltool::interactive::parse_interactive_arguments(tool, output)?
    } else {
        calltool::cmdline::parse_command_line_arguments(args, output)?
    };

    // Call the tool
    let result = client
        .call_tool(tool_name, arguments)
        .timed("   response", output)
        .await?;

    output::calltool::call_tool_result(output, &result)
}

/// Reads a resource from the MCP server.
pub async fn read_resource<C: ClientHandler + 'static>(
    client: &mut Client<C>,
    output: &Output,
    uri: &str,
) -> Result<()> {
    output.text(format!("Reading resource: {uri}"))?;
    let result = client
        .resources_read(uri)
        .timed("    response", output)
        .await?;
    output::readresource::read_resource_result(output, &result)?;
    Ok(())
}

/// Gets a prompt from the MCP server.
pub async fn get_prompt<C: ClientHandler + 'static>(
    client: &mut Client<C>,
    output: &Output,
    name: &str,
    args: Vec<String>,
) -> Result<()> {
    output.text(format!("Getting prompt: {name}"))?;

    // Parse arguments from key=value format
    let arguments = ArgumentParser::parse_key_value_args(args)?;

    let result = client
        .get_prompt(name, arguments)
        .timed("    response", output)
        .await?;
    output::getprompt::get_prompt_result(output, &result)?;
    Ok(())
}

/// Subscribes to resource updates from the MCP server.
pub async fn subscribe_resource<C: ClientHandler + 'static>(
    client: &mut Client<C>,
    output: &Output,
    uri: &str,
) -> Result<()> {
    output.text(format!("Subscribing to resource: {uri}"))?;
    client
        .resources_subscribe(uri)
        .timed("    response", output)
        .await?;
    output.trace_success(format!("Successfully subscribed to resource: {uri}"))?;
    Ok(())
}

/// Unsubscribes from resource updates.
pub async fn unsubscribe_resource<C: ClientHandler + 'static>(
    client: &mut Client<C>,
    output: &Output,
    uri: &str,
) -> Result<()> {
    output.text(format!("Unsubscribing from resource: {uri}"))?;
    client
        .resources_unsubscribe(uri)
        .timed("    response", output)
        .await?;
    output.trace_success(format!("Successfully unsubscribed from resource: {uri}"))?;
    Ok(())
}

/// Gets completions for an argument.
pub async fn complete<C: ClientHandler + 'static>(
    client: &mut Client<C>,
    output: &Output,
    reference: &str,
    argument: &str,
) -> Result<()> {
    output.text(format!("Getting completions for: {reference}/{argument}"))?;

    // Parse the reference into Reference
    let completion_ref = if reference.starts_with("resource://") {
        Reference::Resource(ResourceReference {
            uri: reference.to_string(),
        })
    } else if reference.starts_with("prompt://") {
        Reference::Prompt(PromptReference {
            name: reference
                .strip_prefix("prompt://")
                .unwrap_or(reference)
                .to_string(),
            title: None,
        })
    } else {
        return Err(Error::Other(format!(
            "Invalid reference format: '{}'. Expected resource:// or prompt:// prefix",
            reference
        )));
    };

    let argument_info = ArgumentInfo {
        name: argument.to_string(),
        value: "".to_string(),
    };

    let result = client
        .complete(completion_ref, argument_info)
        .timed("    response", output)
        .await?;
    output::complete::complete_result(output, &result)?;
    Ok(())
}
