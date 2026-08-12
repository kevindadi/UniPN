#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    NotEnabled,
    TypeMismatch,
    Unsupported,
}
