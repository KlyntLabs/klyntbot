use platform_input::{mock::MockInput, ComputerUseAction, KeyMods, PlatformInput};

#[tokio::test]
async fn records_actions_in_arrival_order() {
    let mock = MockInput::new();
    mock.perform_action(ComputerUseAction::MouseMove { x: 100, y: 200 })
        .await
        .unwrap();
    mock.perform_action(ComputerUseAction::LeftClick {
        x: 100,
        y: 200,
        modifiers: KeyMods::default(),
    })
    .await
    .unwrap();

    let recorded = mock.recorded().await;
    assert_eq!(recorded.len(), 2);
    matches!(recorded[0], ComputerUseAction::MouseMove { x: 100, y: 200 });
    matches!(
        recorded[1],
        ComputerUseAction::LeftClick { x: 100, y: 200, .. }
    );
}

#[tokio::test]
async fn cursor_position_reflects_movement() {
    let mock = MockInput::new();
    mock.perform_action(ComputerUseAction::MouseMove { x: 50, y: 75 })
        .await
        .unwrap();
    let pos = mock.get_cursor_position().await.unwrap();
    assert_eq!(pos.x, 50.0);
    assert_eq!(pos.y, 75.0);
}

#[tokio::test]
async fn release_all_does_not_record() {
    let mock = MockInput::new();
    mock.release_all().await.unwrap();
    assert!(mock.recorded().await.is_empty());
}
