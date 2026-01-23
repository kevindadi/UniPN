//! 标识（Marking）模块
//!
//! 表示Petri网在某个时刻的状态

use crate::color::{Token, Multiset};
use crate::error::{CpnError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 标识 - 表示网的状态
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Marking<T: Token> {
    /// 库所ID到多重集的映射
    places: HashMap<String, Multiset<T>>,
}

impl<T: Token> Marking<T> {
    /// 创建空标识
    pub fn new() -> Self {
        Self {
            places: HashMap::new(),
        }
    }
    
    /// 设置库所的token
    pub fn set_tokens(&mut self, place_id: &str, tokens: Multiset<T>) {
        self.places.insert(place_id.to_string(), tokens);
    }
    
    /// 添加token到库所
    pub fn add_tokens(&mut self, place_id: &str, token: T, count: usize) {
        self.places
            .entry(place_id.to_string())
            .or_insert_with(Multiset::new)
            .add(token, count);
    }
    
    /// 从库所移除token
    pub fn remove_tokens(&mut self, place_id: &str, token: &T, count: usize) -> Result<()> {
        let multiset = self.places
            .get_mut(place_id)
            .ok_or_else(|| CpnError::PlaceNotFound(place_id.to_string()))?;
        
        if multiset.remove(token, count) {
            Ok(())
        } else {
            Err(CpnError::InvalidMarking(
                format!("库所 {} 中没有足够的token {}", place_id, token)
            ))
        }
    }
    
    /// 获取库所的token数量
    pub fn get_token_count(&self, place_id: &str, token: &T) -> usize {
        self.places
            .get(place_id)
            .map(|ms| ms.count(token))
            .unwrap_or(0)
    }
    
    /// 获取库所的所有token
    pub fn get_tokens(&self, place_id: &str) -> Option<&Multiset<T>> {
        self.places.get(place_id)
    }
    
    /// 获取库所的可变token引用
    pub fn get_tokens_mut(&mut self, place_id: &str) -> Option<&mut Multiset<T>> {
        self.places.get_mut(place_id)
    }
    
    /// 检查库所是否包含指定数量的token
    pub fn contains_tokens(&self, place_id: &str, token: &T, count: usize) -> bool {
        self.places
            .get(place_id)
            .map(|ms| ms.contains(token, count))
            .unwrap_or(false)
    }
    
    /// 获取库所的总token数
    pub fn get_place_total(&self, place_id: &str) -> usize {
        self.places
            .get(place_id)
            .map(|ms| ms.total())
            .unwrap_or(0)
    }
    
    /// 清空库所
    pub fn clear_place(&mut self, place_id: &str) {
        if let Some(ms) = self.places.get_mut(place_id) {
            ms.clear();
        }
    }
    
    /// 获取所有库所ID
    pub fn place_ids(&self) -> impl Iterator<Item = &String> {
        self.places.keys()
    }
    
    /// 获取所有库所和其token的迭代器
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Multiset<T>)> {
        self.places.iter()
    }
}

impl<T: Token> Default for Marking<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Token> PartialEq for Marking<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.places.len() != other.places.len() {
            return false;
        }
        self.places.iter().all(|(id, ms)| {
            other.places.get(id).map(|other_ms| ms == other_ms).unwrap_or(false)
        })
    }
}

impl<T: Token> Eq for Marking<T> {}

impl<T: Token> std::hash::Hash for Marking<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let mut items: Vec<_> = self.places.iter().collect();
        items.sort_by_key(|(id, _)| *id);
        for (id, ms) in items {
            id.hash(state);
            ms.hash(state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::IntToken;

    #[test]
    fn test_marking_basic_operations() {
        let mut marking = Marking::new();
        
        marking.add_tokens("p1", IntToken(1), 3);
        marking.add_tokens("p1", IntToken(2), 2);
        
        assert_eq!(marking.get_token_count("p1", &IntToken(1)), 3);
        assert_eq!(marking.get_token_count("p1", &IntToken(2)), 2);
        assert_eq!(marking.get_place_total("p1"), 5);
        
        assert!(marking.remove_tokens("p1", &IntToken(1), 2).is_ok());
        assert_eq!(marking.get_token_count("p1", &IntToken(1)), 1);
    }
    
    #[test]
    fn test_marking_equality() {
        let mut m1 = Marking::new();
        let mut m2 = Marking::new();
        
        m1.add_tokens("p1", IntToken(1), 2);
        m2.add_tokens("p1", IntToken(1), 2);
        
        assert_eq!(m1, m2);
        
        m2.add_tokens("p1", IntToken(2), 1);
        assert_ne!(m1, m2);
    }
}
