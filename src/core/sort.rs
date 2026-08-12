use serde::{Deserialize, Serialize};

pub type SortId = usize;
pub type FuncId = usize;
pub type Symbol = String;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sort {
    Unit,
    Bool,
    Bool3,
    Int {
        lo: Option<i64>,
        hi: Option<i64>,
    },
    Enum(Vec<Symbol>),
    Tuple(Vec<SortId>),
    Record(Vec<(Symbol, SortId)>),
    User(Symbol),
}
