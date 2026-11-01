use std::time::{Duration, SystemTime};

use tmcp_http::auth::{OAuth2Client, OAuth2Config, OAuth2Token};

use crate::{Error, Result, ctx::Ctx};

pub async fn renew_command(ctx: &Ctx, name: String) -> Result<()> {
    ctx.output
        .h1(format!("Renewing OAuth authentication: {name}"))?;

    let storage = ctx.storage()?;
    let mut auth = storage.get_auth(&name)?;

    // Check if we have a refresh token
    auth.refresh_token.as_ref().ok_or(Error::Other(
        "No refresh token available for this authentication entry".to_string(),
    ))?;

    ctx.output.text("Current token status:")?;
    match &auth.expires_at {
        Some(expires_at) => {
            let now = SystemTime::now();
            if *expires_at > now {
                let remaining = expires_at.duration_since(now).unwrap_or(Duration::ZERO);
                let hours = remaining.as_secs() / 3600;
                let minutes = (remaining.as_secs() % 3600) / 60;
                ctx.output
                    .text(format!("  Token expires in {hours}h {minutes}m"))?;
            } else {
                ctx.output.text("  Token is expired")?;
            }
        }
        None => {
            ctx.output.text("  No expiration information available")?;
        }
    }

    ctx.output.text("")?;
    ctx.output.text("Refreshing token...")?;

    let client = OAuth2Client::new(OAuth2Config {
        client_id: auth.client_id.clone(),
        client_secret: auth.client_secret.clone(),
        auth_url: auth.auth_url.clone(),
        token_url: auth.token_url.clone(),
        redirect_url: auth
            .redirect_url
            .clone()
            .unwrap_or_else(|| "http://localhost:0".to_owned()),
        resource: String::new(),
        scopes: auth.scopes.clone(),
    })?;
    client
        .set_token(OAuth2Token::from_system_time(
            auth.access_token.clone().unwrap_or_default(),
            auth.refresh_token.clone(),
            auth.expires_at,
        ))
        .await;
    let token = client.refresh_access_token().await?;
    let expires_at = token.system_expires_at();

    // Update the stored auth with new token information
    auth.access_token = Some(token.access_token);
    auth.refresh_token = token.refresh_token;
    auth.expires_at = expires_at;

    // Save the updated auth
    storage.store_auth(&auth)?;

    ctx.output.trace_success("Token refreshed successfully!")?;
    ctx.output.text("")?;
    ctx.output.text("New token status:")?;

    if let Some(expires_at) = auth.expires_at {
        let now = SystemTime::now();
        if expires_at > now {
            let remaining = expires_at.duration_since(now).unwrap_or(Duration::ZERO);
            let hours = remaining.as_secs() / 3600;
            let minutes = (remaining.as_secs() % 3600) / 60;
            ctx.output
                .text(format!("  Token expires in {hours}h {minutes}m"))?;
        } else {
            ctx.output.text("  Token is already expired")?;
        }
    } else {
        ctx.output.text("  No expiration information available")?;
    }

    Ok(())
}
