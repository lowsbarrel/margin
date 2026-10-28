use std::collections::HashMap;
use std::hash::Hash;
use std::mem;

#[derive(Debug)]
pub(crate) struct Generations<K, V> {
    current: HashMap<K, V>,
    previous: HashMap<K, V>,
    evicted: Vec<V>,
    capacity: usize,
}

impl<K: Hash + Eq + Clone, V> Generations<K, V> {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            current: HashMap::new(),
            previous: HashMap::new(),
            evicted: Vec::new(),
            capacity,
        }
    }

    pub(crate) fn get_current(&self, key: &K) -> Option<&V> {
        self.current.get(key)
    }

    pub(crate) fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        if !self.current.contains_key(key)
            && let Some(value) = self.previous.remove(key)
        {
            self.insert(key.clone(), value);
        }
        self.current.get_mut(key)
    }

    pub(crate) fn insert(&mut self, key: K, value: V) {
        if self.current.len() >= self.capacity && !self.current.contains_key(&key) {
            let dropped = mem::replace(&mut self.previous, mem::take(&mut self.current));
            self.evicted.extend(dropped.into_values());
        }
        self.current.insert(key, value);
    }

    pub(crate) const fn has_evicted(&self) -> bool {
        !self.evicted.is_empty()
    }

    pub(crate) fn take_evicted(&mut self) -> Vec<V> {
        mem::take(&mut self.evicted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_used_in_the_last_generation_survive_eviction() {
        let mut cache = Generations::with_capacity(2);
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.insert("c", 3);
        assert_eq!(cache.get_mut(&"a").copied(), Some(1));
        cache.insert("d", 4);
        cache.insert("e", 5);
        assert_eq!(cache.get_mut(&"b").copied(), None);
        assert_eq!(cache.get_mut(&"d").copied(), Some(4));
    }

    #[test]
    fn dropped_generations_are_handed_back_once() {
        let mut cache = Generations::with_capacity(1);
        cache.insert("a", 1);
        cache.insert("b", 2);
        assert!(cache.take_evicted().is_empty());
        cache.insert("c", 3);
        assert_eq!(cache.take_evicted(), vec![1]);
        assert!(cache.take_evicted().is_empty());
    }
}
