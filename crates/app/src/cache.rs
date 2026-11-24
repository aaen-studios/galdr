//! Frames already shown, kept on the GPU so going back over them costs nothing.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use gpui::RenderImage;

struct Entry {
    image: Arc<RenderImage>,
    bytes: u64,
    used: u64,
}

pub struct FrameCache<K> {
    entries: HashMap<K, Entry>,
    bytes: u64,
    budget: u64,
    tick: u64,
}

impl<K: Hash + Eq + Copy> FrameCache<K> {
    pub fn new(budget: u64) -> Self {
        Self { entries: HashMap::new(), bytes: 0, budget, tick: 0 }
    }

    pub fn get(&mut self, index: K) -> Option<Arc<RenderImage>> {
        self.tick += 1;
        let entry = self.entries.get_mut(&index)?;
        entry.used = self.tick;
        Some(entry.image.clone())
    }

    pub fn contains(&self, image: &Arc<RenderImage>) -> bool {
        self.entries.values().any(|e| Arc::ptr_eq(&e.image, image))
    }

    /// Keeps `image` for frame `index`, and gives back what had to go to stay in budget, for the
    /// caller to free on the GPU.
    pub fn insert(&mut self, index: K, image: Arc<RenderImage>, bytes: u64) -> Vec<Arc<RenderImage>> {
        self.tick += 1;
        let mut evicted = Vec::new();
        if let Some(old) = self.entries.insert(index, Entry { image, bytes, used: self.tick }) {
            self.bytes -= old.bytes;
            evicted.push(old.image);
        }
        self.bytes += bytes;
        while self.bytes > self.budget && self.entries.len() > 1 {
            let oldest = *self.entries.iter().min_by_key(|(_, e)| e.used).map(|(k, _)| k).unwrap();
            let e = self.entries.remove(&oldest).unwrap();
            self.bytes -= e.bytes;
            evicted.push(e.image);
        }
        evicted
    }
}
