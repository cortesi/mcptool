use std::time::{Duration, SystemTime};

use rustyline::DefaultEditor;
use tmcp::auth::{OAuth2CallbackServer, OAuth2Client, OAuth2Config};
use tokio::time::timeout;

use crate::{Error, Result, auth::validate_auth_name, ctx::Ctx, storage::StoredAuth};

pub struct AddCommandArgs {
    pub name: String,
    pub server_url: Option<String>,
    pub auth_url: Option<String>,
    pub token_url: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub redirect_url: Option<String>,
    pub resource: Option<String>,
    pub scopes: Option<String>,
    pub show_redirect_url: bool,
}

pub async fn add_command(ctx: &Ctx, args: AddCommandArgs) -> Result<()> {
    let name = args.name;

    // Validate auth name
    validate_auth_name(&name)?;

    ctx.output
        .h1(format!("Adding OAuth authentication entry: {name}"))?;

    // Check if entry already exists
    let storage = ctx.storage()?;
    if storage.list_auth()?.contains(&name) {
        return Err(Error::Other(format!(
            "Authentication entry '{name}' already exists"
        )));
    }

    // Use rustyline for interactive prompts only when needed
    let mut rl = DefaultEditor::new()?;

    // Use provided arguments or prompt for missing values
    let server_url = match args.server_url {
        Some(url) => url,
        None => {
            ctx.output.text("Enter the OAuth provider configuration:")?;
            ctx.output.text("")?;
            rl.readline("Server URL (e.g., https://api.example.com): ")?
        }
    };

    let auth_url = match args.auth_url {
        Some(url) => url,
        None => rl.readline("Authorization URL: ")?,
    };

    let token_url = match args.token_url {
        Some(url) => url,
        None => rl.readline("Token URL: ")?,
    };

    let client_id = match args.client_id {
        Some(id) => id,
        None => rl.readline("Client ID: ")?,
    };

    // Client secret is optional
    let client_secret = match args.client_secret {
        Some(secret) => Some(secret),
        None => {
            let client_secret_input =
                rl.readline("Client Secret (optional, press Enter to skip): ")?;
            if client_secret_input.trim().is_empty() {
                None
            } else {
                Some(client_secret_input)
            }
        }
    };

    // Redirect URL configuration
    let (redirect_url, use_local_server) = match args.redirect_url {
        Some(url) => {
            // Use command line provided redirect URL
            (url, None)
        }
        None => {
            // Use local callback server with common port first
            let callback_port = 8080;
            // Try to bind to port 8080 first (commonly registered), fallback to dynamic
            let actual_port =
                match std::net::TcpListener::bind(format!("127.0.0.1:{callback_port}")) {
                    Ok(_) => callback_port,
                    Err(_) => find_available_port()?,
                };
            let url = format!("http://127.0.0.1:{actual_port}/callback");
            ctx.output.text(format!("Using redirect URL: {url}"))?;
            ctx.output.trace_warn(
                "Note: This URL must be registered in your OAuth application settings!",
            )?;
            (url, Some(actual_port))
        }
    };

    // If user just wants to see the redirect URL, show it and exit
    if args.show_redirect_url {
        ctx.output.text("")?;
        ctx.output.h1("OAuth Redirect URL Information")?;
        ctx.output
            .text(format!("Redirect URL that will be used: {redirect_url}"))?;
        ctx.output.text("")?;
        ctx.output
            .text("Add this URL to your OAuth application settings.")?;
        ctx.output
            .text("Then run the command again without --show-redirect-url to complete setup.")?;
        return Ok(());
    }

    // Resource (audience) - use flag or default
    let resource = args.resource.unwrap_or_default();

    // Scopes - use flag or default to empty
    let scopes: Vec<String> = match args.scopes {
        Some(s) => s.split(',').map(|s| s.trim().to_string()).collect(),
        None => vec![],
    };

    ctx.output.text("")?;
    ctx.output.text("Initiating OAuth flow...")?;

    // Create OAuth configuration
    let oauth_config = OAuth2Config {
        client_id: client_id.clone(),
        client_secret: client_secret.clone(),
        auth_url: auth_url.clone(),
        token_url: token_url.clone(),
        redirect_url: redirect_url.clone(),
        resource: resource.clone(),
        scopes: scopes.clone(),
    };

    // Create OAuth client
    let mut oauth_client = OAuth2Client::new(oauth_config)?;

    // Get authorization URL
    let (auth_url_with_params, csrf_token) = oauth_client.get_authorization_url();

    ctx.output.text("")?;
    ctx.output.h1("Authorization required")?;
    ctx.output
        .text("Please visit the following URL to authorize the application:")?;
    ctx.output.text("")?;
    ctx.output.text(auth_url_with_params.as_str())?;
    ctx.output.text("")?;

    // Try to open browser
    match open::that(auth_url_with_params.as_str()) {
        Ok(_) => {
            ctx.output.trace_success("Browser opened successfully.")?;
            ctx.output
                .text("If the browser didn't open, copy the URL above and paste it manually.")?;
        }
        Err(e) => {
            ctx.output
                .trace_warn(format!("Could not open browser automatically: {e}"))?;
            ctx.output
                .text("Please copy the URL above and open it manually in your browser.")?;

            // Additional troubleshooting hints
            #[cfg(target_os = "macos")]
            ctx.output.text(
                "On macOS: You may need to allow the terminal app to control other applications.",
            )?;
        }
    }

    ctx.output.text("")?;

    // Handle different callback modes
    let token_result = if let Some(callback_port) = use_local_server {
        // Use local callback server
        let callback_server = OAuth2CallbackServer::new(callback_port);

        ctx.output.text("Waiting for authorization callback...")?;
        ctx.output.text(format!(
            "Local callback server listening on port {callback_port}"
        ))?;
        ctx.output
            .text("The browser will redirect back to this server after authorization.")?;
        ctx.output
            .text("Press Ctrl+C to cancel and use manual mode instead.")?;

        // Use tokio::select to handle both callback and cancellation
        tokio::select! {
            result = wait_for_callback(&mut oauth_client, callback_server, csrf_token.secret().to_string()) => {
                Ok(result)
            }
            _ = tokio::signal::ctrl_c() => {
                ctx.output.text("")?;
                ctx.output.trace_warn("Cancelled! Switching to manual mode...")?;
                timeout(
                    Duration::from_secs(300),
                    wait_for_manual_callback(&mut oauth_client, csrf_token.secret().to_string(), &ctx.output),
                ).await
            }
        }
    } else {
        // Manual mode - user will paste callback URL
        ctx.output.text("Manual callback mode:")?;
        ctx.output
            .text("After authorizing, you'll be redirected to your registered URL.")?;
        ctx.output
            .text("Copy the full URL from your browser and paste it when prompted.")?;

        timeout(
            Duration::from_secs(300), // 5 minute timeout
            wait_for_manual_callback(
                &mut oauth_client,
                csrf_token.secret().to_string(),
                &ctx.output,
            ),
        )
        .await
    };

    let token = match token_result {
        Ok(Ok(token)) => token,
        Ok(Err(e)) => {
            let error_msg = format!("{e}");
            if error_msg.contains("redirect_uri") || error_msg.contains("redirect URL") {
                ctx.output.text("")?;
                ctx.output
                    .trace_error("OAuth Error: Redirect URL not registered")?;
                ctx.output
                    .text("The redirect URL is not associated with your OAuth application.")?;
                ctx.output.text("")?;
                ctx.output.text("To fix this:")?;
                ctx.output
                    .text("1. Go to your OAuth application settings")?;
                ctx.output
                    .text(format!("2. Add this redirect URL: {redirect_url}"))?;
                ctx.output.text("3. Run this command again")?;
                ctx.output.text("")?;
                return Err(Error::Other(format!(
                    "OAuth configuration error: {error_msg}"
                )));
            } else if error_msg.contains("incorrect_client_credentials")
                || error_msg.contains("client_id and/or client_secret")
            {
                ctx.output.text("")?;
                ctx.output
                    .trace_error("OAuth Error: Invalid client credentials")?;
                ctx.output
                    .text("The client_id and/or client_secret are incorrect.")?;
                ctx.output.text("")?;
                ctx.output.text("To fix this:")?;
                ctx.output
                    .text("1. Verify your OAuth application settings")?;
                ctx.output
                    .text("2. Make sure the client_id and client_secret match exactly")?;
                ctx.output
                    .text("3. For GitHub: client_secret is required for OAuth Apps")?;
                ctx.output
                    .text("4. Check for trailing spaces or incorrect copy/paste")?;
                ctx.output.text("")?;
                return Err(Error::Other(format!(
                    "OAuth authentication error: {error_msg}"
                )));
            }
            return Err(Error::Other(format!("OAuth error: {error_msg}")));
        }
        Err(_) => {
            return Err(Error::Other(
                "OAuth authorization timed out after 5 minutes".to_string(),
            ));
        }
    };

    ctx.output.trace_success("Authorization successful!")?;

    // Convert token expiration from Instant to SystemTime
    let expires_at = token.expires_at.map(|instant| {
        let duration_since_now = instant.duration_since(std::time::Instant::now());
        SystemTime::now() + duration_since_now
    });

    // Store the authentication
    let stored_auth = StoredAuth {
        name: name.clone(),
        server_url,
        client_id,
        client_secret,
        access_token: Some(token.access_token),
        refresh_token: token.refresh_token,
        expires_at,
        auth_url,
        token_url,
        redirect_url: Some(redirect_url),
        scopes,
    };

    storage.store_auth(&stored_auth)?;

    ctx.output.text("")?;
    ctx.output
        .trace_success(format!("Authentication entry '{name}' saved successfully!"))?;
    ctx.output.text(format!(
        "You can now use: mcptool connect --auth {name} <target>"
    ))?;

    Ok(())
}

async fn wait_for_callback(
    oauth_client: &mut OAuth2Client,
    callback_server: OAuth2CallbackServer,
    expected_state: String,
) -> Result<tmcp::auth::OAuth2Token> {
    // Wait for the OAuth callback
    let (code, state) = callback_server.wait_for_callback().await?;

    // Verify the state parameter matches for CSRF protection
    if state != expected_state {
        return Err(Error::Other(
            "State parameter mismatch - possible CSRF attack".to_string(),
        ));
    }

    // Exchange the authorization code for an access token
    let token = oauth_client.exchange_code(code, state).await?;

    Ok(token)
}

async fn wait_for_manual_callback(
    oauth_client: &mut OAuth2Client,
    expected_state: String,
    output: &crate::output::Output,
) -> Result<tmcp::auth::OAuth2Token> {
    let mut rl = DefaultEditor::new()?;

    output.text("")?;
    let callback_url = rl.readline("Paste the full callback URL from your browser: ")?;

    // Extract the authorization code and state from the callback URL
    let url = url::Url::parse(&callback_url)
        .map_err(|e| Error::Other(format!("Invalid URL format: {e}")))?;

    let mut code = None;
    let mut state = None;
    let mut error = None;
    let mut error_description = None;

    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.to_string()),
            "state" => state = Some(value.to_string()),
            "error" => error = Some(value.to_string()),
            "error_description" => error_description = Some(value.to_string()),
            _ => {}
        }
    }

    // Check for OAuth errors first
    if let Some(error_code) = error {
        let description =
            error_description.unwrap_or_else(|| "No description provided".to_string());
        return Err(Error::Other(format!(
            "OAuth authorization failed: {error_code} - {description}"
        )));
    }

    let code = code.ok_or(Error::Other(
        "No authorization code found in callback URL".to_string(),
    ))?;
    let state = state.ok_or(Error::Other(
        "No state parameter found in callback URL".to_string(),
    ))?;

    // Verify the state parameter matches for CSRF protection
    if state != expected_state {
        return Err(Error::Other(
            "State parameter mismatch - possible CSRF attack".to_string(),
        ));
    }

    // Exchange the authorization code for an access token
    let token = oauth_client
        .exchange_code(code, state)
        .await
        .map_err(|e| Error::Other(format!("Token exchange failed: {e}")))?;

    Ok(token)
}

fn find_available_port() -> Result<u16> {
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    Ok(addr.port())
}

#[cfg(test)]
mod tests {
    use crate::auth::validate_auth_name;

    #[test]
    fn test_validate_auth_name() {
        // Valid names
        assert!(validate_auth_name("myauth").is_ok());
        assert!(validate_auth_name("my_auth").is_ok());
        assert!(validate_auth_name("MyAuth123").is_ok());
        assert!(validate_auth_name("AUTH_123_test").is_ok());
        assert!(validate_auth_name("a").is_ok());
        assert!(validate_auth_name("_").is_ok());
        assert!(validate_auth_name("123").is_ok());

        // Invalid names
        assert!(validate_auth_name("").is_err());
        assert!(validate_auth_name("my-auth").is_err());
        assert!(validate_auth_name("my auth").is_err());
        assert!(validate_auth_name("my:auth").is_err());
        assert!(validate_auth_name("my/auth").is_err());
        assert!(validate_auth_name("my.auth").is_err());
        assert!(validate_auth_name("my@auth").is_err());
        assert!(validate_auth_name("my#auth").is_err());
        assert!(validate_auth_name("my$auth").is_err());
        assert!(validate_auth_name("my%auth").is_err());
        assert!(validate_auth_name("my^auth").is_err());
        assert!(validate_auth_name("my&auth").is_err());
        assert!(validate_auth_name("my*auth").is_err());
        assert!(validate_auth_name("my(auth)").is_err());
        assert!(validate_auth_name("my[auth]").is_err());
        assert!(validate_auth_name("my{auth}").is_err());
        assert!(validate_auth_name("my|auth").is_err());
        assert!(validate_auth_name("my\\auth").is_err());
        assert!(validate_auth_name("my;auth").is_err());
        assert!(validate_auth_name("my'auth").is_err());
        assert!(validate_auth_name("my\"auth").is_err());
        assert!(validate_auth_name("my<auth>").is_err());
        assert!(validate_auth_name("my?auth").is_err());
        assert!(validate_auth_name("my!auth").is_err());
        assert!(validate_auth_name("my~auth").is_err());
        assert!(validate_auth_name("my`auth").is_err());
        assert!(validate_auth_name("my+auth").is_err());
        assert!(validate_auth_name("my=auth").is_err());
        assert!(validate_auth_name("my,auth").is_err());

        // Unicode characters should be invalid
        assert!(validate_auth_name("myαuth").is_err());
        assert!(validate_auth_name("my😀auth").is_err());
        assert!(validate_auth_name("mÿauth").is_err());
    }
}
