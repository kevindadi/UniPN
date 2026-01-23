//! 颜色集和Token系统
//!
//! 提供可扩展的颜色集trait和基本实现

use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::collections::HashMap;

/// Token trait - 定义颜色集中的token
pub trait Token: Clone + Debug + Display + Hash + Eq + Send + Sync {
    /// 获取token的类型名称
    fn type_name(&self) -> &'static str;
    
    /// 检查是否与另一个token兼容
    fn is_compatible(&self, other: &dyn Token) -> bool {
        self.type_name() == other.type_name()
    }
}

/// 颜色集 trait
pub trait ColorSet: Clone + Debug + Send + Sync {
    type TokenType: Token;
    
    /// 获取颜色集的名称
    fn name(&self) -> &str;
    
    /// 检查token是否属于该颜色集
    fn contains(&self, token: &Self::TokenType) -> bool;
    
    /// 获取颜色集的所有可能值（如果可枚举）
    fn enumerate(&self) -> Option<Vec<Self::TokenType>> {
        None
    }
    
    /// 获取颜色集的大小（如果有限）
    fn size(&self) -> Option<usize> {
        self.enumerate().map(|v| v.len())
    }
}

/// Multiset - 多重集，用于表示库所中的token
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Multiset<T: Token> {
    tokens: HashMap<String, (T, usize)>, // key: token的字符串表示, value: (token, 数量)
}

impl<T: Token> Multiset<T> {
    /// 创建空的多重集
    pub fn new() -> Self {
        Self {
            tokens: HashMap::new(),
        }
    }
    
    /// 添加token
    pub fn add(&mut self, token: T, count: usize) {
        let key = format!("{}", token);
        self.tokens
            .entry(key)
            .and_modify(|(_, c)| *c += count)
            .or_insert((token, count));
    }
    
    /// 移除token
    pub fn remove(&mut self, token: &T, count: usize) -> bool {
        let key = format!("{}", token);
        if let Some((_, c)) = self.tokens.get_mut(&key) {
            if *c >= count {
                *c -= count;
                if *c == 0 {
                    self.tokens.remove(&key);
                }
                return true;
            }
        }
        false
    }
    
    /// 获取token数量
    pub fn count(&self, token: &T) -> usize {
        let key = format!("{}", token);
        self.tokens.get(&key).map(|(_, c)| *c).unwrap_or(0)
    }
    
    /// 检查是否包含指定数量的token
    pub fn contains(&self, token: &T, count: usize) -> bool {
        self.count(token) >= count
    }
    
    /// 获取总token数
    pub fn total(&self) -> usize {
        self.tokens.values().map(|(_, c)| c).sum()
    }
    
    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }
    
    /// 清空所有token
    pub fn clear(&mut self) {
        self.tokens.clear();
    }
    
    /// 获取所有token的迭代器
    pub fn iter(&self) -> impl Iterator<Item = (&T, usize)> {
        self.tokens.values().map(|(t, c)| (t, *c))
    }
    
    /// 合并另一个多重集
    pub fn merge(&mut self, other: &Multiset<T>) {
        for (token, count) in other.iter() {
            self.add(token.clone(), count);
        }
    }
    
    /// 检查是否包含另一个多重集
    pub fn contains_multiset(&self, other: &Multiset<T>) -> bool {
        other.iter().all(|(token, count)| self.contains(token, count))
    }
}

impl<T: Token> Default for Multiset<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Token> PartialEq for Multiset<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.tokens.len() != other.tokens.len() {
            return false;
        }
        self.iter().all(|(token, count)| other.count(token) == count)
    }
}

impl<T: Token> Eq for Multiset<T> {}

impl<T: Token> Hash for Multiset<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // 对tokens进行排序后hash，保证相同内容的multiset hash值相同
        let mut items: Vec<_> = self.tokens.keys().collect();
        items.sort();
        for key in items {
            key.hash(state);
            if let Some((_, count)) = self.tokens.get(key) {
                count.hash(state);
            }
        }
    }
}

// ========== 预定义的基本颜色集 ==========

/// 整数Token
#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct IntToken(pub i64);

impl Token for IntToken {
    fn type_name(&self) -> &'static str {
        "Int"
    }
}

impl Display for IntToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 字符串Token
#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct StringToken(pub String);

impl Token for StringToken {
    fn type_name(&self) -> &'static str {
        "String"
    }
}

impl Display for StringToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\"", self.0)
    }
}

/// 布尔Token
#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct BoolToken(pub bool);

impl Token for BoolToken {
    fn type_name(&self) -> &'static str {
        "Bool"
    }
}

impl Display for BoolToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 元组Token - 用于组合多个token
#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct TupleToken<T1: Token, T2: Token>(pub T1, pub T2);

impl<T1: Token, T2: Token> Token for TupleToken<T1, T2> {
    fn type_name(&self) -> &'static str {
        "Tuple"
    }
}

impl<T1: Token, T2: Token> Display for TupleToken<T1, T2> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.0, self.1)
    }
}

/// 整数范围颜色集
#[derive(Clone, Debug)]
pub struct IntRange {
    name: String,
    min: i64,
    max: i64,
}

impl IntRange {
    pub fn new(name: impl Into<String>, min: i64, max: i64) -> Self {
        Self {
            name: name.into(),
            min,
            max,
        }
    }
}

impl ColorSet for IntRange {
    type TokenType = IntToken;
    
    fn name(&self) -> &str {
        &self.name
    }
    
    fn contains(&self, token: &Self::TokenType) -> bool {
        token.0 >= self.min && token.0 <= self.max
    }
    
    fn enumerate(&self) -> Option<Vec<Self::TokenType>> {
        if self.max - self.min > 10000 {
            // 太大的范围不枚举
            return None;
        }
        Some((self.min..=self.max).map(IntToken).collect())
    }
}

/// 枚举颜色集
#[derive(Clone, Debug)]
pub struct EnumColorSet<T: Token> {
    name: String,
    values: Vec<T>,
}

impl<T: Token> EnumColorSet<T> {
    pub fn new(name: impl Into<String>, values: Vec<T>) -> Self {
        Self {
            name: name.into(),
            values,
        }
    }
}

impl<T: Token> ColorSet for EnumColorSet<T> {
    type TokenType = T;
    
    fn name(&self) -> &str {
        &self.name
    }
    
    fn contains(&self, token: &Self::TokenType) -> bool {
        self.values.iter().any(|v| v == token)
    }
    
    fn enumerate(&self) -> Option<Vec<Self::TokenType>> {
        Some(self.values.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiset_basic_operations() {
        let mut ms = Multiset::new();
        let token1 = IntToken(1);
        let token2 = IntToken(2);
        
        ms.add(token1.clone(), 3);
        ms.add(token2.clone(), 2);
        
        assert_eq!(ms.count(&token1), 3);
        assert_eq!(ms.count(&token2), 2);
        assert_eq!(ms.total(), 5);
        
        assert!(ms.remove(&token1, 2));
        assert_eq!(ms.count(&token1), 1);
        
        assert!(!ms.remove(&token1, 5));
        assert_eq!(ms.count(&token1), 1);
    }
    
    #[test]
    fn test_multiset_merge() {
        let mut ms1 = Multiset::new();
        let mut ms2 = Multiset::new();
        
        ms1.add(IntToken(1), 2);
        ms2.add(IntToken(1), 3);
        ms2.add(IntToken(2), 1);
        
        ms1.merge(&ms2);
        
        assert_eq!(ms1.count(&IntToken(1)), 5);
        assert_eq!(ms1.count(&IntToken(2)), 1);
    }
    
    #[test]
    fn test_int_range_color_set() {
        let cs = IntRange::new("Small", 1, 5);
        
        assert!(cs.contains(&IntToken(3)));
        assert!(!cs.contains(&IntToken(0)));
        assert!(!cs.contains(&IntToken(6)));
        
        let values = cs.enumerate().unwrap();
        assert_eq!(values.len(), 5);
    }
}
