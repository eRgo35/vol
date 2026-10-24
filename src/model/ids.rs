//! Object identifiers.

/// Registry global id. Reused by the daemon after removal. Valid only while present in `Store`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId(pub u32);

/// `props["object.serial"]`. Unique for the daemon's lifetime; used for `target.object` and row identity.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Serial(pub u64);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_order_and_hash() {
        assert!(ObjectId(1) < ObjectId(2));
        assert!(Serial(10) > Serial(9));
        let mut ids = vec![ObjectId(3), ObjectId(1), ObjectId(2)];
        ids.sort();
        assert_eq!(ids, [ObjectId(1), ObjectId(2), ObjectId(3)]);

        let set: HashSet<ObjectId> = [ObjectId(5), ObjectId(5), ObjectId(6)].into();
        assert_eq!(set.len(), 2);
        assert!(set.contains(&ObjectId(5)));
        let serials: HashSet<Serial> = [Serial(7), Serial(7)].into();
        assert_eq!(serials.len(), 1);
    }
}
