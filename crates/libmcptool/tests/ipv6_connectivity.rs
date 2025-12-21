use libmcptool::target::Target;
use tokio::{io::AsyncWriteExt, net::TcpListener};

#[tokio::test]
async fn test_ipv6_parsing_and_display() {
    // Test parsing of IPv6 addresses with brackets
    let cases = vec![
        ("[::1]:8080", "::1", 8080),
        ("[2001:db8::1]:443", "2001:db8::1", 443),
        ("[fe80::1%eth0]:3000", "fe80::1%eth0", 3000),
    ];

    for (input, expected_host, expected_port) in cases {
        let target = Target::parse(input).expect("Should parse IPv6 address");
        match target {
            Target::Tcp { ref host, port } => {
                assert_eq!(host, expected_host);
                assert_eq!(port, expected_port);

                // Test display format includes brackets
                let display = target.to_string();
                assert!(display.contains(&format!("[{expected_host}]")));
            }
            _ => panic!("Expected TCP target"),
        }
    }
}

#[tokio::test]
async fn test_ipv6_connectivity() {
    // Try to bind to IPv6 localhost
    let listener = match TcpListener::bind("[::1]:0").await {
        Ok(l) => l,
        Err(_) => {
            eprintln!("Skipping IPv6 test - IPv6 not available on this system");
            return;
        }
    };

    let addr = listener.local_addr().unwrap();
    let port = addr.port();

    // Spawn a simple server
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let _ = socket.write_all(b"IPv6_WORKS").await;
        }
    });

    // Test that we can parse and format IPv6 addresses correctly
    let target_str = format!("[::1]:{port}");
    let target = Target::parse(&target_str).expect("Should parse IPv6 target");

    match target {
        Target::Tcp {
            ref host,
            port: parsed_port,
        } => {
            assert_eq!(host, "::1");
            assert_eq!(parsed_port, port);
        }
        _ => panic!("Expected TCP target"),
    }

    // Verify the display format
    assert_eq!(target.to_string(), format!("tcp://[::1]:{port}"));
}

#[test]
fn test_port_only_format() {
    // Test port-only format
    let test_cases = vec![
        (":8080", "0.0.0.0", 8080),
        (":3000", "0.0.0.0", 3000),
        ("tcp://:9090", "0.0.0.0", 9090),
    ];

    for (input, expected_host, expected_port) in test_cases {
        let target = Target::parse(input).unwrap_or_else(|_| panic!("Should parse: {input}"));
        match target {
            Target::Tcp { ref host, port } => {
                assert_eq!(host, expected_host, "Failed for input: {input}");
                assert_eq!(port, expected_port, "Failed for input: {input}");
            }
            _ => panic!("Expected TCP target for: {input}"),
        }
    }

    // Test that ::1 is not confused with port-only format
    let result = Target::parse("::1");
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "Invalid format: Port is required for TCP targets"
    );
}

#[test]
fn test_ipv6_parsing_edge_cases() {
    // Test various IPv6 address formats
    let valid_cases = vec![
        ("[::]:80", "::", 80),
        ("[::1]:443", "::1", 443),
        ("[2001:db8::]:8080", "2001:db8::", 8080),
        ("[fe80::1%lo0]:3000", "fe80::1%lo0", 3000), // with zone ID
    ];

    for (input, expected_host, expected_port) in valid_cases {
        let target = Target::parse(input).unwrap_or_else(|_| panic!("Should parse: {input}"));
        match target {
            Target::Tcp { ref host, port } => {
                assert_eq!(host, expected_host, "Failed for input: {input}");
                assert_eq!(port, expected_port, "Failed for input: {input}");
            }
            _ => panic!("Expected TCP target for: {input}"),
        }
    }

    // Test invalid cases
    let invalid_cases = vec![
        "[::1]",   // No port
        "[::1]::", // Invalid port
        "[::1",    // Unclosed bracket
        "::1:80",  // IPv6 without brackets should fail with port required error
    ];

    for input in invalid_cases {
        assert!(
            Target::parse(input).is_err(),
            "Should fail to parse: {input}"
        );
    }
}
