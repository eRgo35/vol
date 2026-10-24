//! Audio devices (cards), their profiles and routes.

use super::{ObjectId, PortDir, PropMap, Serial};

/// An `Audio/Device` global.
#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    /// Registry id.
    pub id: ObjectId,
    /// `object.serial`.
    pub serial: Serial,
    /// Properties, verbatim.
    pub props: PropMap,
    /// `EnumProfile` objects.
    pub profiles: Vec<Profile>,
    /// Index of the active profile (`Profile` param).
    pub active_profile: Option<i32>,
    /// `EnumRoute` objects.
    pub routes: Vec<RouteInfo>,
    /// `Route` param objects.
    pub active_routes: Vec<ActiveRoute>,
}

/// True iff `media.class` is exactly `Audio/Device`; only those devices are stored.
pub fn is_audio_device(props: &PropMap) -> bool {
    props
        .get("media.class")
        .is_some_and(|c| c == "Audio/Device")
}

/// One `EnumProfile` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    /// Profile index.
    pub index: i32,
    /// Profile name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Priority; higher is preferred.
    pub priority: i32,
    /// Availability.
    pub available: Availability,
}

/// One `EnumRoute` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteInfo {
    /// Route index.
    pub index: i32,
    /// Direction.
    pub dir: PortDir,
    /// Route name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Priority; higher is preferred.
    pub priority: i32,
    /// Availability.
    pub available: Availability,
    /// Profile indices this route belongs to.
    pub profiles: Vec<i32>,
    /// Card profile devices this route applies to.
    pub devices: Vec<i32>,
}

/// One `Route` param entry (an active route).
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveRoute {
    /// Route index.
    pub index: i32,
    /// The route's `device` field: the card profile device.
    pub card_profile_device: i32,
    /// Direction.
    pub dir: PortDir,
}

/// `SPA_PARAM_AVAILABILITY_*`.
#[derive(Debug, Clone, PartialEq)]
pub enum Availability {
    /// 0.
    Unknown,
    /// 1.
    No,
    /// 2.
    Yes,
}

impl Availability {
    /// Maps the SPA id: 0 unknown, 1 no, 2 yes; any other value is `Unknown`.
    pub fn from_spa(id: u32) -> Self {
        match id {
            1 => Self::No,
            2 => Self::Yes,
            _ => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn availability_from_spa_ids() {
        assert_eq!(Availability::from_spa(0), Availability::Unknown);
        assert_eq!(Availability::from_spa(1), Availability::No);
        assert_eq!(Availability::from_spa(2), Availability::Yes);
    }

    #[test]
    fn only_audio_device_class_is_audio_device() {
        let with_class = |class: &str| PropMap::from([("media.class".into(), class.into())]);
        assert!(is_audio_device(&with_class("Audio/Device")));
        assert!(!is_audio_device(&with_class("Video/Device")));
        assert!(!is_audio_device(&with_class("Audio/Device/Foo")));
        assert!(!is_audio_device(&with_class("")));
        assert!(!is_audio_device(&PropMap::new()));
    }

    #[test]
    fn availability_unknown_for_out_of_range() {
        assert_eq!(Availability::from_spa(3), Availability::Unknown);
        assert_eq!(Availability::from_spa(u32::MAX), Availability::Unknown);
    }
}
