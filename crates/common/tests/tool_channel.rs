use common::tool_channel::{Channel, ChannelMask};

#[test]
fn channel_mask_all_allows_every_channel() {
    let m = ChannelMask::ALL;
    assert!(m.allows(Channel::Coding));
    assert!(m.allows(Channel::Desktop));
    assert!(m.allows(Channel::Other));
}

#[test]
fn channel_mask_coding_only_excludes_others() {
    let m = ChannelMask::CODING_ONLY;
    assert!(m.allows(Channel::Coding));
    assert!(!m.allows(Channel::Desktop));
    assert!(!m.allows(Channel::Other));
}

#[test]
fn channel_mask_non_coding_includes_desktop_and_other() {
    let m = ChannelMask::NON_CODING;
    assert!(!m.allows(Channel::Coding));
    assert!(m.allows(Channel::Desktop));
    assert!(m.allows(Channel::Other));
}

#[test]
fn channel_mask_compose_with_bitor() {
    let m = ChannelMask::CODING | ChannelMask::DESKTOP;
    assert!(m.allows(Channel::Coding));
    assert!(m.allows(Channel::Desktop));
    assert!(!m.allows(Channel::Other));
}

#[test]
fn channel_supports_approval_ui_matches_coding_and_desktop() {
    assert!(Channel::Coding.supports_approval_ui());
    assert!(Channel::Desktop.supports_approval_ui());
    assert!(!Channel::Other.supports_approval_ui());
}
