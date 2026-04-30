use common::coding_channel::{available_for_channel, Channel, CODING_ONLY};

#[test]
fn coding_tools_visible_on_coding_channel() {
    for &name in CODING_ONLY {
        assert!(available_for_channel(name, Channel::Coding), "{name} should be visible in coding mode");
        assert!(!available_for_channel(name, Channel::Desktop), "{name} should be hidden on desktop");
    }
}

#[test]
fn non_coding_tool_visible_everywhere() {
    assert!(available_for_channel("tasks", Channel::Coding));
    assert!(available_for_channel("tasks", Channel::Desktop));
}
