use std::collections::HashMap;

use crate::core::value::Token;
use crate::ids::PlaceId;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Multiset<T> {
    items: Vec<T>,
}

impl<T: PartialEq> Multiset<T> {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn from_items(items: impl IntoIterator<Item = T>) -> Self {
        Self {
            items: items.into_iter().collect(),
        }
    }

    pub fn items(&self) -> &[T] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn insert(&mut self, item: T) {
        self.items.push(item);
    }

    pub fn remove_one(&mut self, item: &T) -> bool {
        let Some(index) = self.items.iter().position(|candidate| candidate == item) else {
            return false;
        };
        self.items.remove(index);
        true
    }

    pub fn contains(&self, item: &T) -> bool {
        self.items.contains(item)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct PtMarking(pub Vec<u64>);

impl PtMarking {
    pub fn new(place_count: usize) -> Self {
        Self(vec![0; place_count])
    }

    pub fn from_tokens(tokens: impl IntoIterator<Item = u64>) -> Self {
        Self(tokens.into_iter().collect())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn tokens(&self, place: PlaceId) -> u64 {
        self.0.get(place.index()).copied().unwrap_or_default()
    }

    pub fn iter(&self) -> impl Iterator<Item = (PlaceId, &u64)> {
        self.0
            .iter()
            .enumerate()
            .map(|(index, tokens)| (PlaceId(index), tokens))
    }

    pub fn iter_nonzero(&self) -> impl Iterator<Item = (PlaceId, u64)> + '_ {
        self.iter()
            .filter_map(|(place, tokens)| (*tokens != 0).then_some((place, *tokens)))
    }

    pub fn set(&mut self, place: PlaceId, tokens: u64) -> bool {
        let Some(slot) = self.0.get_mut(place.index()) else {
            return false;
        };
        *slot = tokens;
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ColoredMarking {
    places: Vec<Multiset<Token>>,
}

impl ColoredMarking {
    pub fn new(place_count: usize) -> Self {
        Self {
            places: vec![Multiset::new(); place_count],
        }
    }

    pub fn place(&self, place: PlaceId) -> Option<&Multiset<Token>> {
        self.places.get(place.index())
    }

    pub fn place_mut(&mut self, place: PlaceId) -> Option<&mut Multiset<Token>> {
        self.places.get_mut(place.index())
    }

    pub fn insert(&mut self, place: PlaceId, token: Token) -> bool {
        let Some(multiset) = self.place_mut(place) else {
            return false;
        };
        multiset.insert(token);
        true
    }

    pub fn remove_one(&mut self, place: PlaceId, token: &Token) -> bool {
        self.place_mut(place)
            .is_some_and(|multiset| multiset.remove_one(token))
    }

    pub fn clear(&mut self, place: PlaceId) -> bool {
        let Some(multiset) = self.place_mut(place) else {
            return false;
        };
        *multiset = Multiset::new();
        true
    }

    pub fn len(&self) -> usize {
        self.places.iter().map(Multiset::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.places.iter().all(Multiset::is_empty)
    }

    pub fn into_places(self) -> Vec<Multiset<Token>> {
        self.places
    }
}

impl std::hash::Hash for Multiset<Token> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.items.hash(state);
    }
}

impl std::hash::Hash for ColoredMarking {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.places.hash(state);
    }
}

pub fn token_counts(marking: &ColoredMarking) -> HashMap<PlaceId, usize> {
    marking
        .places
        .iter()
        .enumerate()
        .map(|(index, place)| (PlaceId(index), place.len()))
        .collect()
}
