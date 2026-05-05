use common::tool_channel::Channel;
use common::CODING_CHANNEL;

#[test]
fn coding_mode_resolves_to_coding_channel() {
    // This is a conceptual test — the actual routing happens inside agent_loop.
    // We verify the helper constants are wired correctly.
    assert_eq!(CODING_CHANNEL, "coding");
}

#[test]
fn channel_from_name_maps_correctly() {
    assert_eq!(Channel::from_name("coding"), Channel::Coding);
    assert_eq!(Channel::from_name("desktop"), Channel::Desktop);
    assert_eq!(Channel::from_name("unknown"), Channel::Other);
}
