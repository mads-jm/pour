use pour::transport::api::ApiClient;

#[test]
fn new_sets_https_base_url() {
    let client = ApiClient::new(27124, "test-key".to_string());
    assert!(
        client.base_url.starts_with("https://"),
        "base_url should use HTTPS scheme, got: {}",
        client.base_url
    );
}

#[test]
fn new_embeds_port_in_base_url() {
    let client = ApiClient::new(12345, "key".to_string());
    assert_eq!(client.base_url, "https://127.0.0.1:12345");
}

#[test]
fn new_uses_default_obsidian_port() {
    let client = ApiClient::new(27124, "key".to_string());
    assert_eq!(client.base_url, "https://127.0.0.1:27124");
}

#[test]
fn new_with_different_ports() {
    for port in [80, 443, 8080, 27124, 65535] {
        let client = ApiClient::new(port, "k".to_string());
        let expected = format!("https://127.0.0.1:{port}");
        assert_eq!(client.base_url, expected);
    }
}

#[tokio::test]
async fn check_connection_returns_false_when_no_server() {
    // Connect to a port where nothing is listening.
    let client = ApiClient::new(19999, "fake-key".to_string());
    let connected = client.check_connection().await;
    assert!(!connected, "should return false when server is unreachable");
}
