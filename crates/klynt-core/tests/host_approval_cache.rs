use klynt_core::approval::host_cache::{HostApprovalCache, HostCheckResult, HostDecision, HostKey};
use std::sync::Arc;

#[test]
fn host_key_normalizes_scheme_and_host() {
    let k = HostKey::from_url("HTTPS://Example.COM:443/path?q=1").unwrap();
    assert_eq!(k.scheme, "https");
    assert_eq!(k.host, "example.com");
    assert_eq!(k.port, 443);
}

#[test]
fn host_key_uses_default_ports() {
    let http = HostKey::from_url("http://example.com/").unwrap();
    assert_eq!(http.port, 80);
    let https = HostKey::from_url("https://example.com/").unwrap();
    assert_eq!(https.port, 443);
}

#[tokio::test]
async fn first_caller_gets_newly_registered() {
    let cache = HostApprovalCache::default();
    let key = HostKey::from_url("https://example.com").unwrap();
    let r = cache.check_or_register(key.clone());
    assert!(matches!(r, HostCheckResult::NewlyRegistered { .. }));
}

#[tokio::test]
async fn second_concurrent_caller_gets_await_pending() {
    let cache = Arc::new(HostApprovalCache::default());
    let key = HostKey::from_url("https://example.com").unwrap();
    let _first = cache.check_or_register(key.clone()); // claims NewlyRegistered
    let r = cache.check_or_register(key);
    assert!(matches!(r, HostCheckResult::AwaitPending(_)));
}

#[tokio::test]
async fn resolve_propagates_to_pending_waiter() {
    let cache = Arc::new(HostApprovalCache::default());
    let key = HostKey::from_url("https://example.com").unwrap();
    let first = cache.check_or_register(key.clone());
    let HostCheckResult::NewlyRegistered { tx } = first else {
        panic!()
    };
    let mut rx = match cache.check_or_register(key.clone()) {
        HostCheckResult::AwaitPending(rx) => rx,
        other => panic!("expected AwaitPending, got {other:?}"),
    };
    tx.send(Some(HostDecision::AllowForSession)).unwrap();
    cache.resolve(key.clone(), HostDecision::AllowForSession);
    rx.changed().await.unwrap();
    assert_eq!(*rx.borrow(), Some(HostDecision::AllowForSession));
    // After resolution, third call returns Cached.
    let third = cache.check_or_register(key);
    assert!(matches!(
        third,
        HostCheckResult::Cached(HostDecision::AllowForSession)
    ));
}

#[tokio::test]
async fn allow_once_evicts_after_resolution() {
    let cache = Arc::new(HostApprovalCache::default());
    let key = HostKey::from_url("https://example.com").unwrap();
    let first = cache.check_or_register(key.clone());
    let HostCheckResult::NewlyRegistered { tx } = first else {
        panic!()
    };
    tx.send(Some(HostDecision::AllowOnce)).unwrap();
    cache.resolve(key.clone(), HostDecision::AllowOnce);
    // After AllowOnce resolution, the key is evicted: next call gets NewlyRegistered.
    let next = cache.check_or_register(key);
    assert!(matches!(next, HostCheckResult::NewlyRegistered { .. }));
}

#[tokio::test]
async fn parallel_calls_to_same_host_share_one_approval() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let cache = Arc::new(HostApprovalCache::default());
    let key = HostKey::from_url("https://shared.example.com").unwrap();

    // Spawn 5 concurrent callers.
    let approval_count = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    for _ in 0..5 {
        let cache_c = cache.clone();
        let key_c = key.clone();
        let approval_count_c = approval_count.clone();
        handles.push(tokio::spawn(async move {
            let r = cache_c.check_or_register(key_c.clone());
            match r {
                HostCheckResult::NewlyRegistered { tx } => {
                    approval_count_c.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    let _ = tx.send(Some(HostDecision::AllowForSession));
                    cache_c.resolve(key_c, HostDecision::AllowForSession);
                    HostDecision::AllowForSession
                }
                HostCheckResult::AwaitPending(mut rx) => {
                    rx.changed().await.unwrap();
                    rx.borrow().unwrap()
                }
                HostCheckResult::Cached(d) => d,
            }
        }));
    }
    for h in handles {
        h.await.unwrap();
    }
    assert_eq!(
        approval_count.load(Ordering::SeqCst),
        1,
        "exactly one approval round-trip should fire for 5 parallel calls"
    );
}
