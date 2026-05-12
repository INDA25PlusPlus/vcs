use dashmap::{DashMap, ReadOnlyView};
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::Deref;

/// Remove all entries in `map` whose keys are not present in at least one other map.
///
/// Example: `remove_difference!(&mut map, &other_a, &other_b)`
///
/// `map.remove(map \ other_0 \ other_1 \ ...)`
macro_rules! remove_difference {
    ($map:expr, $($other:expr),+) => {
        $map.retain(|k, _| {
            false $(|| $other.contains_key(k))+
        })
    };
}
pub(crate) use remove_difference;

pub fn replace_or_insert<K, V>(map: &mut HashMap<K, V>, key: &K, value: V)
where
    K: Eq + Hash + Clone,
{
    if let Some(entry) = map.get_mut(key) {
        *entry = value;
    } else {
        map.insert(key.clone(), value);
    }
}

pub struct DashMapReadOnlyGuard<'a, K, V>
where
    K: Eq + Hash,
{
    map: &'a mut DashMap<K, V>,
    read_only: Option<ReadOnlyView<K, V>>,
}

impl<'a, K, V> DashMapReadOnlyGuard<'a, K, V>
where
    K: Eq + Hash,
{
    pub fn new(map: &'a mut DashMap<K, V>) -> DashMapReadOnlyGuard<'a, K, V> {
        let read_only = std::mem::take(map).into_read_only();
        DashMapReadOnlyGuard {
            map,
            read_only: Some(read_only),
        }
    }
}

impl<'a, K, V> Drop for DashMapReadOnlyGuard<'a, K, V>
where
    K: Eq + Hash,
{
    fn drop(&mut self) {
        if !std::thread::panicking() {
            *self.map = self
                .read_only
                .take()
                .expect("read_only should be Some unless guard has been dropped")
                .into_inner();
        }
    }
}

impl<'a, K, V> Deref for DashMapReadOnlyGuard<'a, K, V>
where
    K: Eq + Hash,
{
    type Target = ReadOnlyView<K, V>;

    fn deref(&self) -> &Self::Target {
        self.read_only
            .as_ref()
            .expect("read_only should be Some unless guard has been dropped")
    }
}

#[derive(Clone, Debug)]
pub enum OuterJoinEntry<VA, VB> {
    Left(VA),
    Right(VB),
    Both(VA, VB),
}

/// Perform an outer join on the values of `map_a` and `map_b`, returning an iterator over all
/// entries which exist in either or both maps.
pub fn outer_join<'a, K, VA, VB>(
    map_a: &'a HashMap<K, VA>,
    map_b: &'a HashMap<K, VB>,
) -> impl Iterator<Item = (&'a K, OuterJoinEntry<&'a VA, &'a VB>)>
where
    K: Eq + Hash,
{
    let left = map_a.iter().map(|(k, va)| match map_b.get(k) {
        None => (k, OuterJoinEntry::Left(va)),
        Some(vb) => (k, OuterJoinEntry::Both(va, vb)),
    });
    let right_outer = map_b.iter().filter_map(|(k, vb)| {
        if map_a.contains_key(k) {
            None
        } else {
            Some((k, OuterJoinEntry::Right(vb)))
        }
    });
    left.chain(right_outer)
}

impl<VA, VB> From<OuterJoinEntry<VA, VB>> for (Option<VA>, Option<VB>) {
    fn from(value: OuterJoinEntry<VA, VB>) -> Self {
        match value {
            OuterJoinEntry::Left(va) => (Some(va), None),
            OuterJoinEntry::Right(vb) => (None, Some(vb)),
            OuterJoinEntry::Both(va, vb) => (Some(va), Some(vb)),
        }
    }
}

impl<VA, VB> From<OuterJoinEntry<VA, VB>> for Option<(VA, VB)> {
    fn from(value: OuterJoinEntry<VA, VB>) -> Self {
        match value {
            OuterJoinEntry::Both(va, vb) => Some((va, vb)),
            _ => None,
        }
    }
}
