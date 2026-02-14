#[tokio::test]
async fn test_reqwest_tls_support() {
    use reqwest::Client;

    // Test that we can build an HTTPS client
    let client_result = Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build();

    assert!(
        client_result.is_ok(),
        "Failed to build HTTP client with TLS: {:?}",
        client_result.err()
    );
}
