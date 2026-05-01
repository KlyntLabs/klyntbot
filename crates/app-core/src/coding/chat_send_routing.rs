use common::{ChannelName, CODING_CHANNEL};

pub fn channel_for_mode(mode_opt: Option<&str>) -> ChannelName {
    match mode_opt {
        Some("coding") => ChannelName::new(CODING_CHANNEL),
        _ => ChannelName::new("desktop"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coding() {
        assert_eq!(channel_for_mode(Some("coding")).as_str(), "coding");
    }
    #[test]
    fn default_desktop() {
        assert_eq!(channel_for_mode(None).as_str(), "desktop");
    }
    #[test]
    fn other_falls_back() {
        assert_eq!(channel_for_mode(Some("chat")).as_str(), "desktop");
    }
}
