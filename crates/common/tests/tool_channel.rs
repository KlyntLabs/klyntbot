use common::tool_channel::{Channel, ChannelMask};

#[test]
fn channel_mask_all_allows_every_channel() {
    let m = ChannelMask::ALL;
    assert!(m.allows(Channel::Desktop));
    assert!(m.allows(Channel::Other));
}

#[test]
fn channel_mask_desktop_only_excludes_others() {
    let m = ChannelMask::DESKTOP_ONLY;
    assert!(m.allows(Channel::Desktop));
    assert!(!m.allows(Channel::Other));
}

#[test]
fn channel_mask_compose_with_bitor() {
    let m = ChannelMask::DESKTOP | ChannelMask::OTHER;
    assert!(m.allows(Channel::Desktop));
    assert!(m.allows(Channel::Other));
}

#[test]
fn channel_supports_approval_ui_matches_desktop() {
    assert!(Channel::Desktop.supports_approval_ui());
    assert!(!Channel::Other.supports_approval_ui());
}
