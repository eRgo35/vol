//! Links between ports.

use super::ObjectId;

/// A link from an output port to an input port.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    /// Registry id.
    pub id: ObjectId,
    /// `link.output.node`.
    pub out_node: ObjectId,
    /// `link.output.port`.
    pub out_port: ObjectId,
    /// `link.input.node`.
    pub in_node: ObjectId,
    /// `link.input.port`.
    pub in_port: ObjectId,
}
