use ai_core_macros::AiEvent;
use bus::CommunityEvent as BusCommunityEvent;
use bus::DomainEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, AiEvent, Serialize, Deserialize)]
#[ai(domain = "General")]
pub enum CommunityEvent {
    #[ai(
        importance = 0.6,
        salience = "accumulate",
        observation_template = "Community discovered: {name} ({member_count} members)"
    )]
    Discovered {
        community_id: String,
        name: String,
        member_count: u32,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Community {name} updated: {reason}"
    )]
    Updated {
        community_id: String,
        name: String,
        reason: String,
    },

    #[ai(
        importance = 0.3,
        salience = "accumulate",
        observation_template = "Community {name} weakened (stability {stability:.2})"
    )]
    Weakened {
        community_id: String,
        name: String,
        stability: f64,
    },
}

impl From<CommunityEvent> for DomainEvent {
    fn from(e: CommunityEvent) -> Self {
        match e {
            CommunityEvent::Discovered {
                community_id,
                name,
                member_count,
            } => DomainEvent::Community(BusCommunityEvent::CommunityDiscovered {
                community_id,
                name,
                member_count,
            }),
            CommunityEvent::Updated {
                community_id,
                name,
                reason,
            } => DomainEvent::Community(BusCommunityEvent::CommunityUpdated {
                community_id,
                name,
                reason,
            }),
            CommunityEvent::Weakened {
                community_id,
                name,
                stability,
            } => DomainEvent::Community(BusCommunityEvent::CommunityWeakened {
                community_id,
                name,
                stability,
            }),
        }
    }
}

/// Translate a bus::DomainEvent into the typed CommunityEvent, if applicable.
pub fn try_from_domain_event(e: &DomainEvent) -> Option<CommunityEvent> {
    match e {
        DomainEvent::Community(BusCommunityEvent::CommunityDiscovered {
            community_id,
            name,
            member_count,
        }) => Some(CommunityEvent::Discovered {
            community_id: community_id.clone(),
            name: name.clone(),
            member_count: *member_count,
        }),
        DomainEvent::Community(BusCommunityEvent::CommunityUpdated {
            community_id,
            name,
            reason,
        }) => Some(CommunityEvent::Updated {
            community_id: community_id.clone(),
            name: name.clone(),
            reason: reason.clone(),
        }),
        DomainEvent::Community(BusCommunityEvent::CommunityWeakened {
            community_id,
            name,
            stability,
        }) => Some(CommunityEvent::Weakened {
            community_id: community_id.clone(),
            name: name.clone(),
            stability: *stability,
        }),
        _ => None,
    }
}
