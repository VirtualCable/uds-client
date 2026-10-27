// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com
//          Janier Rodríguez, jrodriguez at virtualcable dot es

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::raw::{RawCard, RawContext};

pub(crate) struct NativeRegistry {
    pub contexts: ContextTable<RawContext>,
    pub cards: RwLock<HashMap<u64, (RawCard, String)>>,
    pub ctx_cards: RwLock<ContextCards>,
}

impl NativeRegistry {
    pub fn new() -> Self {
        NativeRegistry {
            contexts: ContextTable::default(),
            cards: RwLock::new(HashMap::new()),
            ctx_cards: RwLock::new(ContextCards::default()),
        }
    }
}

impl std::fmt::Debug for NativeRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeRegistry")
            .field("contexts_count", &self.contexts.len())
            .field("cards_count", &self.cards.read().unwrap().len())
            .field("ctx_cards_count", &self.ctx_cards.read().unwrap().len())
            .finish()
    }
}

/// Hands out shared handles so winscard calls never run while the table is locked:
/// a blocking SCardGetStatusChange under a read guard starves every
/// establish/release waiting for the write guard.
pub(crate) struct ContextTable<C> {
    contexts: RwLock<HashMap<u64, Arc<C>>>,
}

impl<C> Default for ContextTable<C> {
    fn default() -> Self {
        ContextTable {
            contexts: RwLock::new(HashMap::new()),
        }
    }
}

impl<C> ContextTable<C> {
    pub fn insert(&self, id: u64, ctx: C) {
        self.contexts.write().unwrap().insert(id, Arc::new(ctx));
    }

    /// Removes the entry; the context itself is dropped by whoever holds the last `Arc`.
    pub fn remove(&self, id: u64) -> Option<Arc<C>> {
        self.contexts.write().unwrap().remove(&id)
    }

    pub fn get(&self, id: u64) -> Option<Arc<C>> {
        self.contexts.read().unwrap().get(&id).cloned()
    }

    pub fn contains(&self, id: u64) -> bool {
        self.contexts.read().unwrap().contains_key(&id)
    }

    pub fn len(&self) -> usize {
        self.contexts.read().unwrap().len()
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    const BLOCKING_WAIT: Duration = Duration::from_millis(400);
    const MAX_WRITER_LATENCY: Duration = Duration::from_millis(100);

    struct DropCounter(Arc<AtomicUsize>);

    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn table_with_one_context() -> (Arc<ContextTable<DropCounter>>, Arc<AtomicUsize>) {
        let drops = Arc::new(AtomicUsize::new(0));
        let table = Arc::new(ContextTable::default());
        table.insert(1, DropCounter(drops.clone()));
        (table, drops)
    }

    fn hold_context_while_waiting(
        table: &Arc<ContextTable<DropCounter>>,
        id: u64,
    ) -> thread::JoinHandle<()> {
        let table = table.clone();
        thread::spawn(move || {
            let ctx = table.get(id).expect("context exists");
            thread::sleep(BLOCKING_WAIT);
            drop(ctx);
        })
    }

    fn assert_quick<T>(what: &str, f: impl FnOnce() -> T) -> T {
        let started = Instant::now();
        let out = f();
        assert!(
            started.elapsed() < MAX_WRITER_LATENCY,
            "{what} waited {:?} behind a held context",
            started.elapsed()
        );
        out
    }

    #[test]
    fn get_returns_none_for_unknown_id() {
        let table: ContextTable<DropCounter> = ContextTable::default();
        assert!(table.get(7).is_none());
        assert!(!table.contains(7));
        assert!(table.remove(7).is_none());
        assert_eq!(table.len(), 0);
    }

    #[test]
    fn establishing_does_not_wait_for_a_context_in_use() {
        let (table, drops) = table_with_one_context();
        let waiter = hold_context_while_waiting(&table, 1);
        thread::sleep(Duration::from_millis(50));

        assert_quick("insert", || table.insert(2, DropCounter(drops.clone())));

        assert_eq!(table.len(), 2);
        waiter.join().unwrap();
    }

    #[test]
    fn releasing_does_not_wait_for_a_context_in_use() {
        let (table, drops) = table_with_one_context();
        let waiter = hold_context_while_waiting(&table, 1);
        thread::sleep(Duration::from_millis(50));

        let removed = assert_quick("remove", || table.remove(1));

        assert!(removed.is_some());
        assert!(!table.contains(1));
        assert_eq!(
            drops.load(Ordering::SeqCst),
            0,
            "still in use by the waiter"
        );
        drop(removed);
        waiter.join().unwrap();
        assert_eq!(
            drops.load(Ordering::SeqCst),
            1,
            "dropped once the waiter let go"
        );
    }

    #[test]
    fn a_removed_context_is_dropped_with_its_last_handle() {
        let (table, drops) = table_with_one_context();
        let held = table.get(1).unwrap();

        drop(table.remove(1));
        assert_eq!(drops.load(Ordering::SeqCst), 0);

        drop(held);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_context_without_outside_handles_is_dropped_on_remove() {
        let (table, drops) = table_with_one_context();

        drop(table.remove(1));

        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

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
