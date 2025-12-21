use std::{collections::HashMap, fs, path::PathBuf, time::SystemTime};

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Entry not found: {0}")]
    NotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAuth {
    pub name: String,
    pub server_url: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<SystemTime>,
    pub auth_url: String,
    pub token_url: String,
    pub redirect_url: Option<String>,
    pub scopes: Vec<String>,
}

pub struct TokenStorage {
    config_dir: PathBuf,
}

impl TokenStorage {
    pub fn new(config_dir: PathBuf) -> Result<Self, StorageError> {
        fs::create_dir_all(&config_dir)?;
        Ok(Self { config_dir })
    }

    pub fn store_auth(&self, auth: &StoredAuth) -> Result<(), StorageError> {
        let auth_path = self.config_dir.join("auth.json");
        let mut all_auths = self.load_all_auth_data()?;
        all_auths.insert(auth.name.clone(), auth.clone());

        let json = serde_json::to_string_pretty(&all_auths)?;
        fs::write(auth_path, json)?;

        Ok(())
    }

    pub fn get_auth(&self, name: &str) -> Result<StoredAuth, StorageError> {
        let all_auths = self.load_all_auth_data()?;
        all_auths
            .get(name)
            .cloned()
            .ok_or_else(|| StorageError::NotFound(name.to_string()))
    }

    pub fn remove_auth(&self, name: &str) -> Result<(), StorageError> {
        let auth_path = self.config_dir.join("auth.json");
        let mut all_auths = self.load_all_auth_data()?;

        if !all_auths.contains_key(name) {
            return Err(StorageError::NotFound(name.to_string()));
        }

        all_auths.remove(name);
        let json = serde_json::to_string_pretty(&all_auths)?;
        fs::write(auth_path, json)?;

        Ok(())
    }

    pub fn list_auth(&self) -> Result<Vec<String>, StorageError> {
        let all_auths = self.load_all_auth_data()?;
        let mut names: Vec<String> = all_auths.keys().cloned().collect();
        names.sort();
        Ok(names)
    }

    pub fn get_all_auth(&self) -> Result<Vec<StoredAuth>, StorageError> {
        let all_auths = self.load_all_auth_data()?;
        let mut auths: Vec<StoredAuth> = all_auths.values().cloned().collect();
        auths.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(auths)
    }

    fn load_all_auth_data(&self) -> Result<HashMap<String, StoredAuth>, StorageError> {
        let auth_path = self.config_dir.join("auth.json");

        if !auth_path.exists() {
            return Ok(HashMap::new());
        }

        let contents = fs::read_to_string(auth_path)?;
        let auths: HashMap<String, StoredAuth> = serde_json::from_str(&contents)?;
        Ok(auths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_lifecycle() {
        let test_dir = std::env::temp_dir().join("mcptool_test").join(format!(
            "test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let storage = TokenStorage::new(test_dir).expect("Failed to create storage");

        // Create test auth
        let auth = StoredAuth {
            name: "test_auth".to_string(),
            server_url: "https://example.com".to_string(),
            client_id: "test_client".to_string(),
            client_secret: Some("test_secret".to_string()),
            access_token: Some("test_token".to_string()),
            refresh_token: Some("test_refresh".to_string()),
            expires_at: Some(SystemTime::now()),
            auth_url: "https://example.com/auth".to_string(),
            token_url: "https://example.com/token".to_string(),
            redirect_url: Some("http://localhost:8080".to_string()),
            scopes: vec!["read".to_string(), "write".to_string()],
        };

        // Store
        storage.store_auth(&auth).expect("Failed to store auth");

        // List
        let names = storage.list_auth().expect("Failed to list auth");
        assert!(names.contains(&"test_auth".to_string()));

        // Get
        let retrieved = storage.get_auth("test_auth").expect("Failed to get auth");
        assert_eq!(retrieved.name, auth.name);
        assert_eq!(retrieved.server_url, auth.server_url);

        // Remove
        storage
            .remove_auth("test_auth")
            .expect("Failed to remove auth");

        // Verify removed
        let names = storage.list_auth().expect("Failed to list auth");
        assert!(!names.contains(&"test_auth".to_string()));
    }
}
