//! OAuth authentication management commands.

/// OAuth credential addition functionality.
mod add;
/// OAuth credential listing functionality.
mod list;
/// OAuth credential removal functionality.
mod remove;
/// OAuth token renewal functionality.
mod renew;

pub use add::{AddCommandArgs, add_command};
pub use list::list_command;
pub use remove::remove_command;
pub use renew::renew_command;

use crate::{Error, Result};

/// Validates that an auth name contains only alphanumeric characters and underscores
pub fn validate_auth_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::Format(
            "Authentication name cannot be empty".to_string(),
        ));
    }

    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(Error::Format(format!(
            "Authentication name '{name}' is invalid. Names can only contain letters, numbers, and underscores (a-zA-Z0-9_)"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

        // Verify error messages
        let err = validate_auth_name("my-auth").unwrap_err();
        assert!(
            err.to_string()
                .contains("Names can only contain letters, numbers, and underscores")
        );

        let err = validate_auth_name("").unwrap_err();
        assert!(err.to_string().contains("cannot be empty"));
    }
}
