use common::available_for_channel;
use common::coding_channel::{Channel, CODING_CHANNEL};

#[test]
fn coding_only_tools() {
    assert!(available_for_channel("bash", Channel::Coding));
    assert!(!available_for_channel("bash", Channel::Desktop));
    assert!(available_for_channel("tasks", Channel::Desktop));
    assert!(available_for_channel("tasks", Channel::Coding));
}
