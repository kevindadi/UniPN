//! ConcIR → CVN.
//!
//! [`ast`] is the wire format, re-declared leniently; [`expr`] turns ConcIR's
//! opaque expression strings into the CVN's guard and update trees; [`lower`]
//! walks a program and builds the net.

pub mod ast;
pub mod expr;
pub mod lower;
