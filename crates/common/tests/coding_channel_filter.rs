use common::tool_channel::{Channel, ChannelMask};

#[test]
fn coding_only_mask_allows_coding_blocks_others() {
    let m = ChannelMask::CODING_ONLY;
    assert!(m.allows(Channel::Coding));
    assert!(!m.allows(Channel::Desktop));
    assert!(!m.allows(Channel::Other));
}

#[test]
fn all_mask_allows_every_channel() {
    let m = ChannelMask::ALL;
    assert!(m.allows(Channel::Coding));
    assert!(m.allows(Channel::Desktop));
    assert!(m.allows(Channel::Other));
}
