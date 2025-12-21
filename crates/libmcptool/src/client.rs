use std::sync::Arc;

use tmcp::auth::{OAuth2Client, OAuth2Config};
use tmcp::{Client, ClientConn, schema::InitializeResult};

use crate::ctx::VERSION;
use crate::{Error, Result, ctx::Ctx, target::Target, utils::TimedFuture};

pub async fn get_client(ctx: &Ctx, target: &Target) -> Result<(Client<()>, InitializeResult)> {
    get_client_with_connection(ctx, target, ()).await
}
pub async fn get_client_with_connection<C: ClientConn + Send + 'static>(
    ctx: &Ctx,
    target: &Target,
    conn: C,
) -> Result<(Client<C>, InitializeResult)> {
    match target {
        Target::Auth { name } => {
            let storage = ctx.storage()?;
            let auth_entry = storage.get_auth(name)?;
            ctx.output
                .text(format!("Using auth {name} ({})", auth_entry.server_url))?;
            let resolved_target = Target::parse(&auth_entry.server_url)?;
            connect_with_auth(ctx, &resolved_target, name, conn)
                .timed("Connected and initialized", &ctx.output)
                .await
        }
        _ => {
            // For other targets, connect directly without auth
            ctx.output.text(format!("Connecting to {target}"))?;
            connect_to_server(target, conn)
                .timed("Connected and initialized", &ctx.output)
                .await
        }
    }
}

async fn connect_with_auth<C: ClientConn + Send + 'static>(
    ctx: &Ctx,
    target: &Target,
    auth_name: &str,
    conn: C,
) -> Result<(Client<C>, InitializeResult)> {
    // Only HTTP/HTTPS targets support OAuth
    match target {
        Target::Http { .. } | Target::Https { .. } => {}
        _ => {
            return Err(Error::Other(
                "OAuth authentication is only supported for HTTP/HTTPS targets".to_string(),
            ));
        }
    }

    let storage = ctx.storage()?;
    let auth = storage.get_auth(auth_name)?;
    if let Some(expires_at) = auth.expires_at
        && expires_at <= std::time::SystemTime::now()
    {
        return Err(Error::Other(
            "Access token has expired. Please re-authenticate with 'mcptool auth add/renew'"
                .to_string(),
        ));
    }

    // Create OAuth config
    let oauth_config = OAuth2Config {
        client_id: auth.client_id,
        client_secret: auth.client_secret,
        auth_url: auth.auth_url,
        token_url: auth.token_url,
        redirect_url: auth
            .redirect_url
            .unwrap_or_else(|| "http://localhost:0".to_string()),
        resource: "".to_string(), // Empty resource, could be stored in auth if needed
        scopes: auth.scopes,
    };

    // Create OAuth client
    let oauth_client = OAuth2Client::new(oauth_config)?;

    // Set the stored tokens if available
    if let Some(access_token) = auth.access_token {
        let token = tmcp::auth::OAuth2Token {
            access_token,
            refresh_token: auth.refresh_token,
            expires_at: auth.expires_at.map(|system_time| {
                // Convert SystemTime to Instant
                match system_time.duration_since(std::time::SystemTime::now()) {
                    Ok(duration) => std::time::Instant::now() + duration,
                    Err(_) => std::time::Instant::now(), // Token is already expired
                }
            }),
        };
        oauth_client.set_token(token).await;
    }

    let oauth_client = Arc::new(oauth_client);

    let mut client = Client::new_with_connection("mcptool", crate::ctx::VERSION, conn);

    let init_result = match target {
        Target::Http { host, port } => {
            let url = format!("http://{host}:{port}");
            client
                .connect_http_with_oauth(&url, oauth_client)
                .await
                .map_err(|e| {
                    Error::Other(format!(
                        "Failed to connect to HTTP endpoint {url} with OAuth: {e}"
                    ))
                })?
        }
        Target::Https { host, port } => {
            let url = format!("https://{host}:{port}");
            client
                .connect_http_with_oauth(&url, oauth_client)
                .await
                .map_err(|e| {
                    Error::Other(format!(
                        "Failed to connect to HTTPS endpoint {url} with OAuth: {e}"
                    ))
                })?
        }
        _ => unreachable!(), // We checked this above
    };

    Ok((client, init_result))
}

pub async fn connect_to_server<C: ClientConn + Send + 'static>(
    target: &Target,
    conn: C,
) -> Result<(Client<C>, InitializeResult)> {
    let mut client = Client::new_with_connection("mcptool", VERSION, conn);

    let init_result = match target {
        Target::Tcp { host, port } => {
            let addr = format!("{host}:{port}");
            client.connect_tcp(&addr).await.map_err(|e| {
                Error::Other(format!("Failed to connect to TCP address {addr}: {e}"))
            })?
        }
        Target::Stdio { command, args } => {
            let mut cmd = tokio::process::Command::new(command);
            cmd.args(args);

            let _child = client
                .connect_process(cmd)
                .await
                .map_err(|e| Error::Other(format!("Failed to spawn MCP server process: {e}")))?;

            // The new API handles initialization automatically
            client
                .init()
                .await
                .map_err(|e| Error::Other(format!("Failed to initialize MCP client: {e}")))?
        }
        Target::Http { host, port } => {
            let url = format!("http://{host}:{port}");
            client.connect_http(&url).await.map_err(|e| {
                Error::Other(format!("Failed to connect to HTTP endpoint {url}: {e}"))
            })?
        }
        Target::Https { host, port } => {
            let url = format!("https://{host}:{port}");
            client.connect_http(&url).await.map_err(|e| {
                Error::Other(format!("Failed to connect to HTTPS endpoint {url}: {e}"))
            })?
        }
        Target::Auth { .. } => {
            return Err(Error::Other(
                "Auth targets should be resolved to actual targets before calling connect_to_server".to_string()
            ));
        }
    };

    Ok((client, init_result))
}
