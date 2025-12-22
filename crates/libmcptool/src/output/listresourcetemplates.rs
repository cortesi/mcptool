use tmcp::schema::ListResourceTemplatesResult;

use crate::Result;
use crate::output::Output;

/// Display the list of resource templates in either JSON or formatted text
pub fn list_resource_templates_result(
    output: &Output,
    templates_result: &ListResourceTemplatesResult,
) -> Result<()> {
    if output.json {
        // Output as JSON
        output.json_value(templates_result)?;
    } else {
        // Output as formatted text
        if templates_result.resource_templates.is_empty() {
            output.text("No resource templates.")?;
        } else {
            for template in &templates_result.resource_templates {
                output.h1(&template.name)?;
                let out = output.indent();

                // Description (if present)
                if let Some(description) = &template.description {
                    out.write_block(description)?;
                }

                // URI Template
                out.kv("URI Template", &template.uri_template)?;

                // Title (if present)
                if let Some(title) = &template.title {
                    out.kv("Title", title)?;
                }

                // MIME Type (if present)
                if let Some(mime_type) = &template.mime_type {
                    out.kv("MIME Type", mime_type)?;
                }

                // Annotations (if present)
                if let Some(annotations) = &template.annotations {
                    out.h2("Annotations")?;
                    let out = out.indent();

                    if let Some(audience) = &annotations.audience {
                        let audience_str = audience
                            .iter()
                            .map(|r| format!("{r:?}"))
                            .collect::<Vec<_>>()
                            .join(", ");
                        out.kv("audience", &audience_str)?;
                    }

                    if let Some(priority) = &annotations.priority {
                        out.kv("priority", priority.to_string())?;
                    }

                    if let Some(last_modified) = &annotations.last_modified {
                        out.kv("last modified", last_modified)?;
                    }
                }

                output.text("")?; // Extra blank line between templates
            }
        }

        // Show cursor information if available
        if let Some(next_cursor) = &templates_result.next_cursor {
            output.note(format!(
                "More resource templates available. Next cursor: {next_cursor}"
            ))?;
        }
    }
    Ok(())
}
