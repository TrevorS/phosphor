// T083 fixture. Not compiled — `tests/fixtures/` has no `main.rs`, so cargo
// ignores the directory as a test target and this file is only ever read as
// bytes by `grammar_abi.rs`.
#![allow(dead_code)]

use std::collections::HashMap;
use std::fmt::{self, Display};
use std::sync::Arc;

pub const MAX_ROWS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Seen {
    Unseen,
    Seen { at: u64 },
}

pub trait Query<'a> {
    type Output: Display;
    fn run(&'a self, needle: &str) -> Option<Self::Output>;
}

#[derive(Debug, Default)]
pub struct Store<T: Send + 'static> {
    rows: Vec<T>,
    index: HashMap<String, usize>,
}

impl<T: Send + Display + 'static> Store<T> {
    pub fn new() -> Self {
        Self { rows: Vec::new(), index: HashMap::new() }
    }

    pub fn push(&mut self, key: impl Into<String>, row: T) -> usize {
        let id = self.rows.len();
        self.index.insert(key.into(), id);
        self.rows.push(row);
        id
    }

    pub fn get(&self, key: &str) -> Option<&T> {
        let Some(&id) = self.index.get(key) else {
            return None;
        };
        self.rows.get(id)
    }
}

impl fmt::Display for Seen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Seen::Unseen => write!(f, "●"),
            Seen::Seen { at } => write!(f, "seen@{at}"),
        }
    }
}

pub async fn drain<S>(mut source: S, budget: usize) -> Arc<[String]>
where
    S: Iterator<Item = String> + Unpin,
{
    let mut out = Vec::with_capacity(budget);
    while let Some(item) = source.next() {
        if out.len() >= budget {
            break;
        }
        out.push(item);
    }
    out.into()
}

macro_rules! glyph {
    ($name:ident => $s:literal) => {
        pub const $name: &str = $s;
    };
}

glyph!(THINKING => "✻");
glyph!(NEEDS_YOU => "!");

fn shapes(n: u32) -> &'static str {
    match n {
        0 => "none",
        1..=9 => "few",
        _ if n % 2 == 0 => "many-even",
        _ => "many",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_round_trips() {
        let mut s: Store<String> = Store::new();
        s.push("a", "alpha".to_owned());
        assert_eq!(s.get("a").map(String::as_str), Some("alpha"));
        assert_eq!(shapes(4), "few");
    }
}
