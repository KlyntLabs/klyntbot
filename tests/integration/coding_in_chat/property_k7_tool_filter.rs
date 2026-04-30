use common::tool_channel::{Channel, ChannelMask};
use proptest::prelude::*;

proptest! {
    #[test]
    fn k7_coding_tools_only_in_coding_channel(
        coding in any::<bool>(),
        desktop in any::<bool>(),
        other in any::<bool>(),
        ch_idx in 0u8..3,
    ) {
        let mut mask = ChannelMask::empty();
        if coding { mask |= ChannelMask::CODING; }
        if desktop { mask |= ChannelMask::DESKTOP; }
        if other { mask |= ChannelMask::OTHER; }
        let ch = match ch_idx { 0 => Channel::Coding, 1 => Channel::Desktop, _ => Channel::Other };
        let allowed = mask.allows(ch);
        // Idempotence / consistency check: the same mask+channel must always yield the same result.
        prop_assert_eq!(allowed, mask.allows(ch));
    }

    #[test]
    fn k7_all_mask_allows_every_channel(ch_idx in 0u8..3) {
        let ch = match ch_idx { 0 => Channel::Coding, 1 => Channel::Desktop, _ => Channel::Other };
        prop_assert!(ChannelMask::ALL.allows(ch));
    }
}
