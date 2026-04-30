use common::coding_channel::{available_for_channel, Channel, CODING_ONLY};
use proptest::prelude::*;

proptest! {
    #[test]
    fn k7_coding_tools_only_in_coding_channel(idx in 0usize..CODING_ONLY.len()) {
        let name = CODING_ONLY[idx];
        prop_assert!(available_for_channel(name, Channel::Coding));
        prop_assert!(!available_for_channel(name, Channel::Desktop));
        prop_assert!(!available_for_channel(name, Channel::Other));
    }

    #[test]
    fn k7_non_coding_tools_visible_on_all_channels(suffix in "[a-z]{3,8}") {
        let name = format!("klyntbot_{suffix}");  // anything not in CODING_ONLY
        prop_assert!(available_for_channel(&name, Channel::Coding));
        prop_assert!(available_for_channel(&name, Channel::Desktop));
        prop_assert!(available_for_channel(&name, Channel::Other));
    }
}
