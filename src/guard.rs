//! Guard条件系统
//!
//! 定义变迁的保护条件

use crate::color::Token;
use crate::marking::Marking;
use std::fmt::Debug;

/// Guard trait - 变迁的保护条件
pub trait Guard<T: Token>: Debug + Send + Sync {
    /// 检查Guard条件是否满足
    /// 
    /// # 参数
    /// * `binding` - 变量绑定，将变量名映射到token
    /// * `marking` - 当前网的标识
    fn evaluate(&self, binding: &std::collections::HashMap<String, T>, marking: &Marking<T>) -> bool;
    
    /// 克隆Guard
    fn clone_box(&self) -> Box<dyn Guard<T>>;
}

/// 始终为真的Guard
#[derive(Debug, Clone)]
pub struct AlwaysTrue;

impl<T: Token> Guard<T> for AlwaysTrue {
    fn evaluate(&self, _binding: &std::collections::HashMap<String, T>, _marking: &Marking<T>) -> bool {
        true
    }
    
    fn clone_box(&self) -> Box<dyn Guard<T>> {
        Box::new(self.clone())
    }
}

/// 始终为假的Guard
#[derive(Debug, Clone)]
pub struct AlwaysFalse;

impl<T: Token> Guard<T> for AlwaysFalse {
    fn evaluate(&self, _binding: &std::collections::HashMap<String, T>, _marking: &Marking<T>) -> bool {
        false
    }
    
    fn clone_box(&self) -> Box<dyn Guard<T>> {
        Box::new(self.clone())
    }
}

/// 自定义Guard - 使用闭包
pub struct CustomGuard<T: Token> {
    name: String,
    predicate: Box<dyn Fn(&std::collections::HashMap<String, T>, &Marking<T>) -> bool + Send + Sync>,
}

impl<T: Token> CustomGuard<T> {
    pub fn new<F>(name: impl Into<String>, predicate: F) -> Self
    where
        F: Fn(&std::collections::HashMap<String, T>, &Marking<T>) -> bool + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            predicate: Box::new(predicate),
        }
    }
}

impl<T: Token> Debug for CustomGuard<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CustomGuard({})", self.name)
    }
}

impl<T: Token> Guard<T> for CustomGuard<T> {
    fn evaluate(&self, binding: &std::collections::HashMap<String, T>, marking: &Marking<T>) -> bool {
        (self.predicate)(binding, marking)
    }
    
    fn clone_box(&self) -> Box<dyn Guard<T>> {
        panic!("CustomGuard cannot be cloned due to closure")
    }
}

/// And Guard - 逻辑与
#[derive(Debug)]
pub struct AndGuard<T: Token> {
    guards: Vec<Box<dyn Guard<T>>>,
}

impl<T: Token> AndGuard<T> {
    pub fn new(guards: Vec<Box<dyn Guard<T>>>) -> Self {
        Self { guards }
    }
}

impl<T: Token> Guard<T> for AndGuard<T> {
    fn evaluate(&self, binding: &std::collections::HashMap<String, T>, marking: &Marking<T>) -> bool {
        self.guards.iter().all(|g| g.evaluate(binding, marking))
    }
    
    fn clone_box(&self) -> Box<dyn Guard<T>> {
        Box::new(Self {
            guards: self.guards.iter().map(|g| g.clone_box()).collect(),
        })
    }
}

/// Or Guard - 逻辑或
#[derive(Debug)]
pub struct OrGuard<T: Token> {
    guards: Vec<Box<dyn Guard<T>>>,
}

impl<T: Token> OrGuard<T> {
    pub fn new(guards: Vec<Box<dyn Guard<T>>>) -> Self {
        Self { guards }
    }
}

impl<T: Token> Guard<T> for OrGuard<T> {
    fn evaluate(&self, binding: &std::collections::HashMap<String, T>, marking: &Marking<T>) -> bool {
        self.guards.iter().any(|g| g.evaluate(binding, marking))
    }
    
    fn clone_box(&self) -> Box<dyn Guard<T>> {
        Box::new(Self {
            guards: self.guards.iter().map(|g| g.clone_box()).collect(),
        })
    }
}

/// Not Guard - 逻辑非
#[derive(Debug)]
pub struct NotGuard<T: Token> {
    guard: Box<dyn Guard<T>>,
}

impl<T: Token> NotGuard<T> {
    pub fn new(guard: Box<dyn Guard<T>>) -> Self {
        Self { guard }
    }
}

impl<T: Token> Guard<T> for NotGuard<T> {
    fn evaluate(&self, binding: &std::collections::HashMap<String, T>, marking: &Marking<T>) -> bool {
        !self.guard.evaluate(binding, marking)
    }
    
    fn clone_box(&self) -> Box<dyn Guard<T>> {
        Box::new(Self {
            guard: self.guard.clone_box(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::IntToken;
    use std::collections::HashMap;

    #[test]
    fn test_always_true_guard() {
        let guard = AlwaysTrue;
        let binding = HashMap::new();
        let marking = Marking::new();
        
        assert!(guard.evaluate(&binding, &marking));
    }
    
    #[test]
    fn test_always_false_guard() {
        let guard = AlwaysFalse;
        let binding = HashMap::new();
        let marking = Marking::new();
        
        assert!(!guard.evaluate(&binding, &marking));
    }
    
    #[test]
    fn test_and_guard() {
        let guard = AndGuard::new(vec![
            Box::new(AlwaysTrue),
            Box::new(AlwaysTrue),
        ]);
        let binding = HashMap::new();
        let marking = Marking::new();
        
        assert!(guard.evaluate(&binding, &marking));
        
        let guard2 = AndGuard::new(vec![
            Box::new(AlwaysTrue),
            Box::new(AlwaysFalse),
        ]);
        assert!(!guard2.evaluate(&binding, &marking));
    }
}
