#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Multiset<T>(pub Vec<T>);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct PtMarking(pub Vec<u32>);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct ColoredMarking(pub Vec<Multiset<crate::core::value::Token>>);
