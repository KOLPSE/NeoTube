//! Cola de reproducción y reglas de "siguiente".
//!
//! La cola es **local** y no se sincroniza con la cuenta (PLAN.md §5): las
//! listas de la cuenta se abren, se reproducen y se copian a la cola, pero la
//! cola no se escribe de vuelta salvo que el usuario lo pida.

use crate::domain::{Card, MediaKind, QueueItem, RepeatMode};
use crate::store::Store;

#[derive(Debug, Default)]
pub struct Queue {
    items: Vec<QueueItem>,
    /// Índice del elemento en curso; `None` si no se ha empezado.
    current: Option<usize>,
    repeat: RepeatMode,
    shuffle: bool,
    /// Orden aleatorio estable para que "atrás" funcione con shuffle activo.
    shuffled_order: Vec<usize>,
}

impl Queue {
    pub fn new(repeat: RepeatMode) -> Self {
        Self {
            repeat,
            ..Default::default()
        }
    }

    pub fn items(&self) -> &[QueueItem] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    pub fn current(&self) -> Option<&QueueItem> {
        self.current.and_then(|i| self.items.get(i))
    }

    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    pub fn set_repeat(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }

    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    pub fn set_shuffle(&mut self, on: bool) {
        self.shuffle = on;
        if on {
            self.rebuild_shuffle();
        } else {
            self.shuffled_order.clear();
        }
    }

    fn rebuild_shuffle(&mut self) {
        // Barajado determinista por reloj: no se usa `rand` para no arrastrar
        // otra dependencia, y la calidad estadística da igual aquí.
        let mut order: Vec<usize> = (0..self.items.len()).collect();
        let mut seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15)
            | 1;
        for i in (1..order.len()).rev() {
            // xorshift64*
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let j = (seed % (i as u64 + 1)) as usize;
            order.swap(i, j);
        }
        self.shuffled_order = order;
    }

    /// Reemplaza la cola y empieza por el primer elemento.
    pub fn replace(&mut self, items: Vec<QueueItem>, start_at: usize) -> Option<QueueItem> {
        self.items = items;
        self.current = None;
        if self.shuffle {
            self.rebuild_shuffle();
        }
        self.jump_to(start_at)
    }

    /// Fija la cola sin cambiar lo que suena.
    pub fn set_items(&mut self, items: Vec<QueueItem>) {
        self.items = items;
        if self.shuffle {
            self.rebuild_shuffle();
        }
        if let Some(i) = self.current {
            if i >= self.items.len() {
                self.current = self.items.len().checked_sub(1);
            }
        }
    }

    pub fn push(&mut self, item: QueueItem) {
        self.items.push(item);
        if self.shuffle {
            self.rebuild_shuffle();
        }
    }

    /// Añade una tarjeta si es reproducible. Devuelve si se añadió.
    pub fn push_card(&mut self, card: &Card) -> bool {
        match QueueItem::from_card(card) {
            Some(item) => {
                self.push(item);
                true
            }
            None => false,
        }
    }

    /// Añade todos los elementos reproducibles de una tarjeta-lista.
    pub fn extend_cards(&mut self, cards: impl IntoIterator<Item = Card>) -> usize {
        let mut added = 0;
        for card in cards {
            if matches!(
                card.kind,
                MediaKind::Video | MediaKind::Short | MediaKind::Live | MediaKind::Song
            ) {
                if self.push_card(&card) {
                    added += 1;
                }
            }
        }
        added
    }

    pub fn jump_to(&mut self, index: usize) -> Option<QueueItem> {
        let item = self.items.get(index)?.clone();
        self.current = Some(index);
        Some(item)
    }

    /// Siguiente elemento según repetición y aleatorio.
    pub fn peek_next(&self) -> Option<(usize, QueueItem)> {
        if self.items.is_empty() {
            return None;
        }
        let position = self.current_index_in_order();
        let next_in_order = position.map(|p| p + 1).unwrap_or(0);
        if next_in_order < self.order_len() {
            let index = self.order_at(next_in_order);
            return self.items.get(index).map(|item| (index, item.clone()));
        }
        // Final de la cola: solo se sigue si se repite la cola.
        match self.repeat {
            RepeatMode::All => {
                if self.shuffle && self.shuffled_order.is_empty() {
                    return None;
                }
                let index = self.order_at(0);
                self.items.get(index).map(|item| (index, item.clone()))
            }
            _ => None,
        }
    }

    pub fn advance(&mut self) -> Option<QueueItem> {
        if self.repeat == RepeatMode::One {
            if let Some(item) = self.current() {
                return Some(item.clone());
            }
        }
        let (index, item) = self.peek_next()?;
        self.current = Some(index);
        Some(item)
    }

    pub fn peek_previous(&self) -> Option<(usize, QueueItem)> {
        let position = self.current_index_in_order()?;
        if position == 0 {
            let last = self.order_len().checked_sub(1)?;
            let index = self.order_at(last);
            return self.items.get(index).map(|item| (index, item.clone()));
        }
        let index = self.order_at(position - 1);
        self.items.get(index).map(|item| (index, item.clone()))
    }

    pub fn previous(&mut self) -> Option<QueueItem> {
        let (index, item) = self.peek_previous()?;
        self.current = Some(index);
        Some(item)
    }

    /// Orden efectivo: el natural o el barajado.
    fn order_len(&self) -> usize {
        if self.shuffle && !self.shuffled_order.is_empty() {
            self.shuffled_order.len()
        } else {
            self.items.len()
        }
    }

    fn order_at(&self, position: usize) -> usize {
        if self.shuffle && !self.shuffled_order.is_empty() {
            self.shuffled_order[position.min(self.shuffled_order.len() - 1)]
        } else {
            position
        }
    }

    fn current_index_in_order(&self) -> Option<usize> {
        let current = self.current?;
        if self.shuffle && !self.shuffled_order.is_empty() {
            self.shuffled_order.iter().position(|i| *i == current)
        } else {
            Some(current)
        }
    }

    pub fn persist(&self, store: &Store) {
        store.save_queue(&self.items);
    }

    pub fn restore(&mut self, store: &Store) {
        let items = store.load_queue();
        if !items.is_empty() {
            self.items = items;
            self.current = Some(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Thumbs;

    fn item(id: &str) -> QueueItem {
        QueueItem {
            video_id: id.into(),
            title: id.into(),
            author: "c".into(),
            duration: None,
            thumbs: Thumbs::default(),
            kind: MediaKind::Song,
        }
    }

    #[test]
    fn siguiente_y_anterior_sin_repeticion() {
        let mut q = Queue::new(RepeatMode::Off);
        q.replace(vec![item("a"), item("b"), item("c")], 0);
        assert_eq!(q.advance().unwrap().video_id, "b");
        assert_eq!(q.advance().unwrap().video_id, "c");
        assert!(q.advance().is_none(), "sin repetir, la cola se acaba");
        assert_eq!(q.previous().unwrap().video_id, "b");
    }

    #[test]
    fn repeticion_de_cola_vuelve_al_principio() {
        let mut q = Queue::new(RepeatMode::All);
        q.replace(vec![item("a"), item("b")], 1);
        assert_eq!(q.advance().unwrap().video_id, "a");
    }

    #[test]
    fn repeticion_de_una_repite_la_misma() {
        let mut q = Queue::new(RepeatMode::One);
        q.replace(vec![item("a"), item("b")], 0);
        assert_eq!(q.advance().unwrap().video_id, "a");
    }

    #[test]
    fn aleatorio_cubre_toda_la_cola() {
        let mut q = Queue::new(RepeatMode::All);
        q.replace((0..12).map(|i| item(&format!("v{i}"))).collect(), 0);
        q.set_shuffle(true);
        let mut vistos = std::collections::HashSet::new();
        for _ in 0..12 {
            if let Some(next) = q.advance() {
                vistos.insert(next.video_id);
            }
        }
        assert_eq!(vistos.len(), 12, "el aleatorio no debe repetir ni saltarse");
    }
}
