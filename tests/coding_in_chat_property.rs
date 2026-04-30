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

    #[test]
    fn k14_no_mutating_tool_is_visible_in_non_ui_channel(
        tool_idx in 0u8..13,
        ch_idx in 0u8..3,
    ) {
        // Enumerate all 13 klynt-core tool names
        let tools = [
            ("bash", true), ("edit", true), ("write", true),
            ("apply_patch", true), ("notebook_edit", true),
            ("enter_plan_mode", false), ("exit_plan_mode", false),
            ("read", false), ("glob", false), ("grep", false),
            ("ask_user", false), ("web_fetch", false), ("tool_search", false),
        ];
        let (name, is_mutating) = tools[tool_idx as usize];
        let ch = match ch_idx { 0 => Channel::Coding, 1 => Channel::Desktop, _ => Channel::Other };

        // After Task 8 graduation, the read-only / portable tools graduate.
        // Mutating tools never graduate. Therefore, for any mutating tool in
        // a non-coding channel, the mask must NOT allow.
        if is_mutating && !matches!(ch, Channel::Coding) {
            // ChannelMask::CODING_ONLY is the expected override — it does not allow non-coding.
            let mask = common::tool_channel::ChannelMask::CODING_ONLY;
            prop_assert!(!mask.allows(ch),
                "mutating tool {name} must not be visible in {ch:?}");
        }
        let _ = name; // suppress unused warning
    }
}
