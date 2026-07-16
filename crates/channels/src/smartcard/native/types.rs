// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::collections::HashMap;
use std::sync::RwLock;

pub(crate) struct NativeRegistry {
    pub contexts: RwLock<HashMap<u64, pcsc::Context>>,
    pub cards: RwLock<HashMap<u64, (pcsc::Card, String)>>,
}

impl NativeRegistry {
    pub fn new() -> Self {
        NativeRegistry {
            contexts: RwLock::new(HashMap::new()),
            cards: RwLock::new(HashMap::new()),
        }
    }
}

impl std::fmt::Debug for NativeRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeRegistry")
            .field("contexts_count", &self.contexts.read().unwrap().len())
            .field("cards_count", &self.cards.read().unwrap().len())
            .finish()
    }
}
