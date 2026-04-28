use platform_capture::{
    mock::MockCapture, AccessibilityNode, AxScope, PlatformCapture,
};
use platform_input::Rect;
use std::collections::HashMap;

#[tokio::test]
async fn returns_fixture_frame() {
    let mock = MockCapture::new();
    let frame = MockCapture::checkerboard_frame();
    mock.set_frame(frame.clone()).await;

    let captured = mock.capture_screen(None).await.unwrap();
    assert_eq!(captured.width, 4);
    assert_eq!(captured.height, 4);
    assert_eq!(captured.data, frame.data);
}

#[tokio::test]
async fn returns_fixture_ax_tree() {
    let mock = MockCapture::new();
    let tree = AccessibilityNode {
        role: "AXWindow".into(),
        label: Some("Test".into()),
        value: None,
        frame: Rect { x: 0.0, y: 0.0, w: 800.0, h: 600.0 },
        children: vec![],
        attrs: HashMap::new(),
    };
    mock.set_ax_tree(tree.clone()).await;

    let got = mock.get_ax_tree(AxScope::ActiveApp).await.unwrap();
    assert_eq!(got.role, "AXWindow");
    assert_eq!(got.label.as_deref(), Some("Test"));
}

#[tokio::test]
async fn empty_capture_returns_error() {
    let mock = MockCapture::new();
    let result = mock.capture_screen(None).await;
    assert!(result.is_err());
}
