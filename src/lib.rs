//! # CPN - 着色Petri网框架
//!
//! 一个功能完整的着色Petri网（Colored Petri Net）实现，支持：
//! - 可扩展的颜色集系统
//! - Guard trait用于变迁保护条件
//! - 抑制弧和重置弧
//! - 矩阵和图两种表示方式
//! - 可达图生成和分析
//! - Graphviz图形化输出
//!
//! ## 示例
//!
//! ```rust
//! use cpn::prelude::*;
//!
//! // 创建一个简单的着色Petri网
//! let mut net = PetriNet::new("simple_net");
//! ```

pub mod color;
pub mod guard;
pub mod arc;
pub mod place;
pub mod transition;
pub mod net;
pub mod marking;
pub mod reachability;
pub mod visualization;
pub mod error;

pub mod prelude {
    //! 常用类型和trait的预导入模块
    pub use crate::color::{ColorSet, Token, Multiset};
    pub use crate::guard::{Guard, AlwaysTrue, CustomGuard};
    pub use crate::arc::{Arc, ArcType, ArcExpression};
    pub use crate::place::Place;
    pub use crate::transition::Transition;
    pub use crate::net::{PetriNet, NetBuilder};
    pub use crate::marking::Marking;
    pub use crate::reachability::{ReachabilityGraph, ReachabilityAnalyzer};
    pub use crate::visualization::Visualizer;
    pub use crate::error::{CpnError, Result};
}
