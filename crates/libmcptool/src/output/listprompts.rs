use crate::Result;
use crate::output::Output;

/// Display the list of prompts in either JSON or formatted text
pub fn list_prompts_result(
    output: &Output,
    prompts_result: &tmcp::schema::ListPromptsResult,
) -> Result<()> {
    if output.json {
        // Output as JSON
        output.json_value(prompts_result)?;
    } else {
        // Output as formatted text
        if prompts_result.prompts.is_empty() {
            output.text("No prompts.")?;
        } else {
            for prompt in &prompts_result.prompts {
                output.h1(&prompt.name)?;

                let out = output.indent();

                // Description
                if let Some(description) = &prompt.description {
                    for line in description.lines() {
                        out.text(line)?;
                    }
                }

                // Arguments
                if let Some(arguments) = &prompt.arguments
                    && !arguments.is_empty()
                {
                    let out = out.indent();
                    out.h2("Arguments")?;
                    let out = out.indent();

                    for arg in arguments {
                        // Use the argument name from the struct
                        out.kv(&arg.name, "")?;

                        let out = out.indent();

                        // Show required marker if required
                        if let Some(true) = arg.required {
                            out.note("[required]")?;
                        }

                        // Show description if available
                        if let Some(description) = &arg.description {
                            out.text(description)?;
                        }
                    }
                }
                output.text("")?; // Extra blank line between prompts
            }
        }

        // Show cursor information if available
        if let Some(next_cursor) = &prompts_result.next_cursor {
            output.note(format!(
                "More prompts available. Next cursor: {next_cursor}"
            ))?;
        }
    }
    Ok(())
}
