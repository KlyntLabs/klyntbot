use common::tool_channel::{Channel, ChannelMask};

#[test]
fn coding_tools_visible_on_coding_channel() {
    // Verify that CODING_ONLY mask behaves correctly — this is the mask
    // returned by klynt-core mutating tools via Tool::allowed_channels().
    let mask = ChannelMask::CODING_ONLY;
    assert!(
        mask.allows(Channel::Coding),
        "coding should be visible in coding mode"
    );
    assert!(
        !mask.allows(Channel::Desktop),
        "coding should be hidden on desktop"
    );
    assert!(
        !mask.allows(Channel::Other),
        "coding should be hidden on other channels"
    );
}

#[test]
fn non_coding_tool_visible_everywhere() {
    // Default mask (ALL) allows every channel.
    let mask = ChannelMask::ALL;
    assert!(mask.allows(Channel::Coding));
    assert!(mask.allows(Channel::Desktop));
    assert!(mask.allows(Channel::Other));
}

#[test]
fn assistant_tools_hidden_in_coding() {
    // NON_CODING mask is returned by assistant-only feature tools.
    let mask = ChannelMask::NON_CODING;
    assert!(
        !mask.allows(Channel::Coding),
        "assistant tools should be hidden in coding mode"
    );
    assert!(
        mask.allows(Channel::Desktop),
        "assistant tools should be visible on desktop"
    );
    assert!(
        mask.allows(Channel::Other),
        "assistant tools should be visible on other channels"
    );
}
