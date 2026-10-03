use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SpaceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemId(pub u64);

impl std::fmt::Display for SpaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "space#{}", self.0)
    }
}

impl std::fmt::Display for ItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "item#{}", self.0)
    }
}

#[derive(Debug, Clone, Default)]
pub struct IdGenerator {
    space: u64,
    item: u64,
}

impl IdGenerator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn next_space(&mut self) -> SpaceId {
        self.space += 1;
        SpaceId(self.space)
    }

    pub fn next_item(&mut self) -> ItemId {
        self.item += 1;
        ItemId(self.item)
    }

    pub fn observe_space(&mut self, id: SpaceId) {
        self.space = self.space.max(id.0);
    }

    pub fn observe_item(&mut self, id: ItemId) {
        self.item = self.item.max(id.0);
    }

    pub fn seed(space: u64, item: u64) -> Self {
        Self { space, item }
    }
}
