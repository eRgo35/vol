//! Node ports.

use super::ObjectId;

/// Port direction (`port.direction`).
#[derive(Debug, Clone, PartialEq)]
pub enum PortDir {
    /// `"in"`.
    In,
    /// `"out"`.
    Out,
}

impl PortDir {
    /// Parses `props["port.direction"]`: `"in"` or `"out"`; anything else is `None`.
    pub fn from_prop(value: &str) -> Option<Self> {
        match value {
            "in" => Some(Self::In),
            "out" => Some(Self::Out),
            _ => None,
        }
    }
}

/// A port of a node.
#[derive(Debug, Clone, PartialEq)]
pub struct Port {
    /// Registry id.
    pub id: ObjectId,
    /// Owning node (`node.id`).
    pub node: ObjectId,
    /// `port.direction`.
    pub dir: PortDir,
    /// `audio.channel`.
    pub channel: Option<String>,
    /// `port.monitor`.
    pub monitor: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_dir_from_prop() {
        assert_eq!(PortDir::from_prop("in"), Some(PortDir::In));
        assert_eq!(PortDir::from_prop("out"), Some(PortDir::Out));
        assert_eq!(PortDir::from_prop(""), None);
        assert_eq!(PortDir::from_prop("In"), None);
        assert_eq!(PortDir::from_prop("output"), None);
    }
}
