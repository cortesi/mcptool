use std::time::{Duration, SystemTime};

use libmcptool::storage::{StorageError, StoredAuth, TokenStorage};

fn create_test_storage() -> TokenStorage {
    let test_dir = std::env::temp_dir().join("mcptool_test").join(format!(
        "test_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    TokenStorage::new(test_dir).expect("Failed to create test storage")
}

#[test]
fn test_auth_storage_lifecycle() {
    // Create a unique test auth entry
    let test_name = format!("test_auth_{}", std::process::id());

    let storage = create_test_storage();

    // Ensure test entry doesn't exist
    _ = storage.remove_auth(&test_name);

    // Create test auth
    let auth = StoredAuth {
        name: test_name.clone(),
        server_url: "https://test.example.com".to_string(),
        client_id: "test_client_id".to_string(),
        client_secret: Some("test_secret".to_string()),
        access_token: Some("test_access_token".to_string()),
        refresh_token: Some("test_refresh_token".to_string()),
        expires_at: Some(SystemTime::now() + Duration::from_secs(3600)),
        auth_url: "https://test.example.com/auth".to_string(),
        token_url: "https://test.example.com/token".to_string(),
        redirect_url: Some("http://localhost:8080".to_string()),
        scopes: vec!["read".to_string(), "write".to_string()],
    };

    // Test store
    storage.store_auth(&auth).expect("Failed to store auth");

    // Test list
    let names = storage.list_auth().expect("Failed to list auth");
    assert!(names.contains(&test_name), "Auth entry not found in list");

    // Test get
    let retrieved = storage.get_auth(&test_name).expect("Failed to get auth");
    assert_eq!(retrieved.name, auth.name);
    assert_eq!(retrieved.server_url, auth.server_url);
    assert_eq!(retrieved.client_id, auth.client_id);
    assert_eq!(retrieved.client_secret, auth.client_secret);
    assert!(retrieved.access_token.is_some());
    assert_eq!(retrieved.refresh_token, auth.refresh_token);
    assert_eq!(retrieved.auth_url, auth.auth_url);
    assert_eq!(retrieved.token_url, auth.token_url);
    assert_eq!(retrieved.redirect_url, auth.redirect_url);
    assert_eq!(retrieved.scopes, auth.scopes);

    // Test remove
    storage
        .remove_auth(&test_name)
        .expect("Failed to remove auth");

    // Verify removed
    match storage.get_auth(&test_name) {
        Err(StorageError::NotFound(_)) => {} // Expected
        Ok(_) => panic!("Auth entry should have been removed"),
        Err(e) => panic!("Unexpected error: {e}"),
    }

    // Test list after removal
    let names = storage
        .list_auth()
        .expect("Failed to list auth after removal");
    assert!(
        !names.contains(&test_name),
        "Auth entry still in list after removal"
    );
}

#[test]
fn test_auth_not_found() {
    let storage = create_test_storage();

    match storage.get_auth("nonexistent_auth_entry") {
        Err(StorageError::NotFound(_)) => {} // Expected
        Ok(_) => panic!("Should not find nonexistent auth"),
        Err(e) => panic!("Unexpected error: {e}"),
    }
}

#[test]
fn test_remove_nonexistent() {
    let storage = create_test_storage();

    match storage.remove_auth("nonexistent_auth_entry") {
        Err(StorageError::NotFound(_)) => {} // Expected
        Ok(_) => panic!("Should not be able to remove nonexistent auth"),
        Err(e) => panic!("Unexpected error: {e}"),
    }
}
