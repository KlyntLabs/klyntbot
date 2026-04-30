use common::tool_channel::{Channel, ChannelMask};
use proptest::prelude::*;

proptest! {
    #[test]
    fn k12_channel_mask_filter_idempotent(
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
        let pass1 = mask.allows(ch);
        let pass2 = mask.allows(ch);
        prop_assert_eq!(pass1, pass2);
    }
}
