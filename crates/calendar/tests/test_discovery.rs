//! Integration test for CalDAV URL discovery

use calendar::CalDavClient;

#[tokio::test]
#[ignore] // Requires real credentials
async fn test_icloud_discovery() {
    let result = CalDavClient::discover_calendar_url(
        "https://caldav.icloud.com/",
        "jayden.dangvu@gmail.com",
        "nekq-mfdq-jwan-whdm",
        "Personal",
    )
    .await;

    match &result {
        Ok(url) => {
            eprintln!("✓ Discovery succeeded!");
            eprintln!("  Discovered URL: {}", url);
        }
        Err(e) => {
            eprintln!("✗ Discovery failed!");
            eprintln!("  Error: {:?}", e);
        }
    }

    assert!(result.is_ok(), "Discovery should succeed with valid credentials");
}
