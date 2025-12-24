use tmcp::schema::ListResourcesResult;

use crate::{Result, output::Output};

/// Display the list of resources in either JSON or formatted text
pub fn list_resources_result(
    output: &Output,
    resources_result: &ListResourcesResult,
) -> Result<()> {
    if output.json {
        // Output as JSON
        output.json_value(resources_result)?;
    } else {
        // Output as formatted text
        if resources_result.resources.is_empty() {
            output.text("No resources.")?;
        } else {
            for resource in &resources_result.resources {
                output.h1(&resource.uri)?;
                output.text("")?; // Extra blank line between resources

                let out = output.indent();

                // Description
                if let Some(description) = &resource.description {
                    for line in description.lines() {
                        out.text(line)?;
                    }
                    out.text("")?;
                }

                // MIME type
                if let Some(mime_type) = &resource.mime_type {
                    out.kv("MIME type", mime_type)?;
                }

                // Size
                if let Some(size) = &resource.size {
                    out.kv("Size", format!("{size} bytes"))?;
                }

                // Annotations
                if let Some(annotations) = &resource.annotations {
                    let out = out.indent();
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

                output.text("")?; // Extra blank line between resources
            }
        }

        // Show cursor information if available
        if let Some(next_cursor) = &resources_result.next_cursor {
            output.note(format!(
                "More resources available. Next cursor: {next_cursor}"
            ))?;
        }
    }
    Ok(())
}
