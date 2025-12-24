use tmcp::schema::InitializeResult;

use crate::{Result, output::Output};

/// Display the initialization result in either JSON or formatted text
pub fn init_result(output: &Output, init_result: &InitializeResult) -> Result<()> {
    if output.json {
        // Output as JSON
        output.json_value(init_result)?;
    } else {
        let title = format!(
            "{} ({})",
            &init_result.server_info.name, &init_result.server_info.version
        );

        // Output as formatted text
        output.h1(title)?;

        let out = output.indent();

        // Protocol version
        out.kv("MCP Protocol Version", &init_result.protocol_version)?;
        if let Some(title) = &init_result.server_info.title {
            out.kv("Title", title)?;
        }

        // Server capabilities
        out.h2("Capabilities")?;
        {
            let out = out.indent();

            let mut unsupported = Vec::new();

            if let Some(tools) = &init_result.capabilities.tools {
                out.success("tools")?;
                if tools.list_changed.unwrap_or(false) {
                    let out = out.indent();
                    out.success("- list changed")?;
                }
            } else {
                unsupported.push("tools");
            }

            // Resources
            if let Some(resources) = &init_result.capabilities.resources {
                out.success("resources")?;
                let out = out.indent();
                if resources.list_changed.unwrap_or(false) {
                    out.success("- list changed")?;
                }
                if resources.subscribe.unwrap_or(false) {
                    out.success("- subscribe")?;
                }
            } else {
                unsupported.push("resources");
            }

            // Prompts
            if let Some(prompts) = &init_result.capabilities.prompts {
                out.success("prompts")?;
                if prompts.list_changed.unwrap_or(false) {
                    let out = out.indent();
                    out.success("- list changed")?;
                }
            } else {
                unsupported.push("prompts");
            }

            // Logging
            if init_result.capabilities.logging.is_some() {
                out.success("logging")?;
            } else {
                unsupported.push("logging");
            }

            // Completions
            if init_result.capabilities.completions.is_some() {
                out.success("completions")?;
            } else {
                unsupported.push("completions");
            }

            // Display unsupported capabilities
            if !unsupported.is_empty() {
                let unsupported_str = unsupported
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                out.text(format!("unsupported: {unsupported_str}"))?;
            }

            // Experimental capabilities
            if let Some(experimental) = &init_result.capabilities.experimental
                && !experimental.is_empty()
            {
                out.text("")?;
                out.h3("Experimental Features")?;
                let out = out.indent();
                for (key, value) in experimental {
                    // Format the value as a pretty JSON string
                    let value_str =
                        serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
                    out.kv(key, &value_str)?;
                }
            }
        }

        // Instructions (if present)
        if let Some(instructions) = &init_result.instructions {
            out.h2("Instructions")?;
            let out = out.indent();
            for line in instructions.lines() {
                out.text(line)?;
            }
        }
        output.text("")?; // Extra blank line at the end
    }
    Ok(())
}
