//! 错误处理模块
//!
//! 定义了CPN框架中所有可能的错误类型

use thiserror::Error;

/// CPN框架的错误类型
#[derive(Error, Debug, Clone)]
pub enum CpnError {
    /// 库所不存在
    #[error("库所 '{0}' 不存在")]
    PlaceNotFound(String),

    /// 变迁不存在
    #[error("变迁 '{0}' 不存在")]
    TransitionNotFound(String),

    /// 弧不存在
    #[error("弧 '{0}' 不存在")]
    ArcNotFound(String),

    /// 变迁不可激发
    #[error("变迁 '{0}' 不可激发")]
    TransitionNotEnabled(String),

    /// 颜色类型不匹配
    #[error("颜色类型不匹配: 期望 {expected}, 实际 {actual}")]
    ColorTypeMismatch {
        expected: String,
        actual: String,
    },

    /// Guard条件不满足
    #[error("变迁 '{0}' 的Guard条件不满足")]
    GuardConditionFailed(String),

    /// 无效的标识
    #[error("无效的标识: {0}")]
    InvalidMarking(String),

    /// 矩阵维度不匹配
    #[error("矩阵维度不匹配: {0}")]
    MatrixDimensionMismatch(String),

    /// 图构建错误
    #[error("图构建错误: {0}")]
    GraphBuildError(String),

    /// 可达性分析错误
    ##[error("可达性分析错误: {0}")]
    ReachabilityError(String),

    /// 可视化错误
    #[error("可视化错误: {0}")]
    VisualizationError(String),

    /// 序列化/反序列化错误
    #[error("序列化错误: {0}")]
    SerializationError(String),

    /// 通用错误
    #[error("{0}")]
    Generic(String),
}

/// Result类型别名
pub type Result<T> = std::result::Result<T, CpnError>;
