//! Values from the `default` metadata object.

/// Default devices, as `node.name`s.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Defaults {
    /// Effective default sink (`default.audio.sink`).
    pub sink: Option<String>,
    /// Effective default source (`default.audio.source`).
    pub source: Option<String>,
    /// User-configured default sink (`default.configured.audio.sink`).
    pub configured_sink: Option<String>,
    /// User-configured default source (`default.configured.audio.source`).
    pub configured_source: Option<String>,
}

/// Per-stream routing target (`target.object` / `target.node`, subject = stream node id).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StreamTarget {
    /// `target.object`.
    pub object: Option<String>,
    /// `target.node`.
    pub node: Option<String>,
}
