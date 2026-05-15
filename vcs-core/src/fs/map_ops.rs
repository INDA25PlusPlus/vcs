use dashmap::{DashMap, ReadOnlyView};
use std::hash::Hash;
use std::ops::{Deref, DerefMut};

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

#[inline]
pub fn replace_or_insert<K, V>(map: &DashMap<K, V>, key: &K, value: V)
where
    K: Eq + Hash + Clone,
{
    if let Some(mut entry) = map.get_mut(key) {
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
    #[inline]
    pub fn new(map: &'a mut DashMap<K, V>) -> DashMapReadOnlyGuard<'a, K, V> {
        let replaced = std::mem::replace(map, DashMap::new());
        DashMapReadOnlyGuard {
            map,
            read_only: Some(replaced.into_read_only()),
        }
    }
}

impl<'a, K, V> Drop for DashMapReadOnlyGuard<'a, K, V>
where
    K: Eq + Hash,
{
    #[inline]
    fn drop(&mut self) {
        if let Some(replaced) = self.read_only.take() {
            *self.map = replaced.into_inner();
        } else {
            // avoid double panic
            if !std::thread::panicking() {
                panic!("read_only should be Some unless guard has been dropped");
            }
        }
    }
}

impl<'a, K, V> Deref for DashMapReadOnlyGuard<'a, K, V>
where
    K: Eq + Hash,
{
    type Target = ReadOnlyView<K, V>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.read_only
            .as_ref()
            .expect("read_only should be Some unless guard has been dropped")
    }
}

impl<'a, K, V> DerefMut for DashMapReadOnlyGuard<'a, K, V>
where
    K: Eq + Hash,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.read_only
            .as_mut()
            .expect("read_only should be Some unless guard has been dropped")
    }
}

pub struct DashMapGuard<'a, K, V>
where
    K: Eq + Hash,
{
    read_only: &'a mut ReadOnlyView<K, V>,
    map: Option<DashMap<K, V>>,
}

impl<'a, K, V> DashMapGuard<'a, K, V>
where
    K: Eq + Hash,
{
    #[inline]
    pub fn new(read_only: &'a mut ReadOnlyView<K, V>) -> DashMapGuard<'a, K, V> {
        let replaced = std::mem::replace(read_only, DashMap::new().into_read_only());
        DashMapGuard {
            read_only,
            map: Some(replaced.into_inner()),
        }
    }
}

impl<'a, K, V> Drop for DashMapGuard<'a, K, V>
where
    K: Eq + Hash,
{
    #[inline]
    fn drop(&mut self) {
        if let Some(replaced) = self.map.take() {
            *self.read_only = replaced.into_read_only();
        } else {
            // avoid double panic
            if !std::thread::panicking() {
                panic!("map should be Some unless guard has been dropped");
            }
        }
    }
}

impl<'a, K, V> Deref for DashMapGuard<'a, K, V>
where
    K: Eq + Hash,
{
    type Target = DashMap<K, V>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.map
            .as_ref()
            .expect("map should be Some unless guard has been dropped")
    }
}

impl<'a, K, V> DerefMut for DashMapGuard<'a, K, V>
where
    K: Eq + Hash,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.map
            .as_mut()
            .expect("map should be Some unless guard has been dropped")
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
    map_a: &'a ReadOnlyView<K, VA>,
    map_b: &'a ReadOnlyView<K, VB>,
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
    #[inline]
    fn from(value: OuterJoinEntry<VA, VB>) -> Self {
        match value {
            OuterJoinEntry::Left(va) => (Some(va), None),
            OuterJoinEntry::Right(vb) => (None, Some(vb)),
            OuterJoinEntry::Both(va, vb) => (Some(va), Some(vb)),
        }
    }
}

impl<VA, VB> From<OuterJoinEntry<VA, VB>> for Option<(VA, VB)> {
    #[inline]
    fn from(value: OuterJoinEntry<VA, VB>) -> Self {
        match value {
            OuterJoinEntry::Both(va, vb) => Some((va, vb)),
            _ => None,
        }
    }
}
