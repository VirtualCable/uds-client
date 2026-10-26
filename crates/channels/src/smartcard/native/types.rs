// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com
//          Janier Rodríguez, jrodriguez at virtualcable dot es

use std::collections::HashMap;
use std::sync::RwLock;

use super::raw::{RawCard, RawContext};

pub(crate) struct NativeRegistry {
    pub contexts: RwLock<HashMap<u64, RawContext>>,
    pub cards: RwLock<HashMap<u64, (RawCard, String)>>,
    pub ctx_cards: RwLock<ContextCards>,
}

impl NativeRegistry {
    pub fn new() -> Self {
        NativeRegistry {
            contexts: RwLock::new(HashMap::new()),
            cards: RwLock::new(HashMap::new()),
            ctx_cards: RwLock::new(ContextCards::default()),
        }
    }
}

impl std::fmt::Debug for NativeRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeRegistry")
            .field("contexts_count", &self.contexts.read().unwrap().len())
            .field("cards_count", &self.cards.read().unwrap().len())
            .field("ctx_cards_count", &self.ctx_cards.read().unwrap().len())
            .finish()
    }
}

/// Which card handles were connected through each context.
#[derive(Default)]
pub(crate) struct ContextCards {
    cards_by_context: HashMap<u64, Vec<u64>>,
}

impl ContextCards {
    pub fn add(&mut self, ctx_id: u64, card_id: u64) {
        self.cards_by_context
            .entry(ctx_id)
            .or_default()
            .push(card_id);
    }

    pub fn remove_card(&mut self, card_id: u64) {
        for cards in self.cards_by_context.values_mut() {
            cards.retain(|id| *id != card_id);
        }
    }

    pub fn take_cards(&mut self, ctx_id: u64) -> Vec<u64> {
        self.cards_by_context.remove(&ctx_id).unwrap_or_default()
    }

    pub fn first_card(&self, ctx_id: u64) -> Option<u64> {
        self.cards_by_context
            .get(&ctx_id)
            .and_then(|cards| cards.first().copied())
    }

    pub fn len(&self) -> usize {
        self.cards_by_context.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn releasing_a_context_yields_every_card_connected_in_it() {
        let mut cards = ContextCards::default();
        cards.add(1, 10);
        cards.add(1, 11);
        cards.add(2, 20);

        assert_eq!(cards.take_cards(1), vec![10, 11]);
        assert_eq!(cards.first_card(1), None);
        assert_eq!(cards.first_card(2), Some(20));
        assert!(cards.take_cards(1).is_empty());
    }

    #[test]
    fn disconnecting_one_card_keeps_the_others() {
        let mut cards = ContextCards::default();
        cards.add(1, 10);
        cards.add(1, 11);

        cards.remove_card(10);

        assert_eq!(cards.first_card(1), Some(11));
        assert_eq!(cards.take_cards(1), vec![11]);
    }
}
