//! Behavior tree framework for AI decision making
//!
//! This module provides a generic behavior tree implementation that can be used
//! to create complex AI behaviors from simple building blocks.

use serde::{Deserialize, Serialize};

/// Result of a behavior tree node execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BehaviorStatus {
    /// The behavior succeeded
    Success,
    /// The behavior failed
    Failure,
    /// The behavior is still running and needs more ticks
    Running,
}

/// Context for behavior tree execution
///
/// This can be extended with game-specific data needed by behaviors
pub trait BehaviorContext: std::any::Any {
    /// Update the context (called each tick)
    fn update(&mut self);
}

/// A node in the behavior tree
pub trait BehaviorNode: Send + Sync {
    /// Execute this node with the given context
    fn tick(&mut self, context: &mut dyn BehaviorContext) -> BehaviorStatus;
    
    /// Reset the node state
    fn reset(&mut self);
}

/// Selector node - succeeds if any child succeeds (OR logic)
///
/// Tries children in order until one succeeds. If all fail, returns Failure.
pub struct Selector {
    children: Vec<Box<dyn BehaviorNode>>,
    current_child: usize,
}

impl Selector {
    pub fn new(children: Vec<Box<dyn BehaviorNode>>) -> Self {
        Self {
            children,
            current_child: 0,
        }
    }
}

impl BehaviorNode for Selector {
    fn tick(&mut self, context: &mut dyn BehaviorContext) -> BehaviorStatus {
        while self.current_child < self.children.len() {
            let status = self.children[self.current_child].tick(context);
            
            match status {
                BehaviorStatus::Success => {
                    self.reset();
                    return BehaviorStatus::Success;
                }
                BehaviorStatus::Running => {
                    return BehaviorStatus::Running;
                }
                BehaviorStatus::Failure => {
                    self.current_child += 1;
                }
            }
        }
        
        self.reset();
        BehaviorStatus::Failure
    }
    
    fn reset(&mut self) {
        self.current_child = 0;
        for child in &mut self.children {
            child.reset();
        }
    }
}

/// Sequence node - succeeds if all children succeed (AND logic)
///
/// Tries children in order. If any fails, returns Failure.
pub struct Sequence {
    children: Vec<Box<dyn BehaviorNode>>,
    current_child: usize,
}

impl Sequence {
    pub fn new(children: Vec<Box<dyn BehaviorNode>>) -> Self {
        Self {
            children,
            current_child: 0,
        }
    }
}

impl BehaviorNode for Sequence {
    fn tick(&mut self, context: &mut dyn BehaviorContext) -> BehaviorStatus {
        while self.current_child < self.children.len() {
            let status = self.children[self.current_child].tick(context);
            
            match status {
                BehaviorStatus::Success => {
                    self.current_child += 1;
                }
                BehaviorStatus::Running => {
                    return BehaviorStatus::Running;
                }
                BehaviorStatus::Failure => {
                    self.reset();
                    return BehaviorStatus::Failure;
                }
            }
        }
        
        self.reset();
        BehaviorStatus::Success
    }
    
    fn reset(&mut self) {
        self.current_child = 0;
        for child in &mut self.children {
            child.reset();
        }
    }
}

/// Inverter node - inverts the result of its child
pub struct Inverter {
    child: Box<dyn BehaviorNode>,
}

impl Inverter {
    pub fn new(child: Box<dyn BehaviorNode>) -> Self {
        Self { child }
    }
}

impl BehaviorNode for Inverter {
    fn tick(&mut self, context: &mut dyn BehaviorContext) -> BehaviorStatus {
        match self.child.tick(context) {
            BehaviorStatus::Success => BehaviorStatus::Failure,
            BehaviorStatus::Failure => BehaviorStatus::Success,
            BehaviorStatus::Running => BehaviorStatus::Running,
        }
    }
    
    fn reset(&mut self) {
        self.child.reset();
    }
}

/// Repeater node - repeats its child a certain number of times
pub struct Repeater {
    child: Box<dyn BehaviorNode>,
    max_repeats: usize,
    current_repeats: usize,
}

impl Repeater {
    pub fn new(child: Box<dyn BehaviorNode>, max_repeats: usize) -> Self {
        Self {
            child,
            max_repeats,
            current_repeats: 0,
        }
    }
}

impl BehaviorNode for Repeater {
    fn tick(&mut self, context: &mut dyn BehaviorContext) -> BehaviorStatus {
        while self.current_repeats < self.max_repeats {
            let status = self.child.tick(context);
            
            match status {
                BehaviorStatus::Running => {
                    return BehaviorStatus::Running;
                }
                BehaviorStatus::Success | BehaviorStatus::Failure => {
                    self.current_repeats += 1;
                    self.child.reset();
                }
            }
        }
        
        self.reset();
        BehaviorStatus::Success
    }
    
    fn reset(&mut self) {
        self.current_repeats = 0;
        self.child.reset();
    }
}

/// Succeeder node - always returns Success
pub struct Succeeder {
    child: Box<dyn BehaviorNode>,
}

impl Succeeder {
    pub fn new(child: Box<dyn BehaviorNode>) -> Self {
        Self { child }
    }
}

impl BehaviorNode for Succeeder {
    fn tick(&mut self, context: &mut dyn BehaviorContext) -> BehaviorStatus {
        let _ = self.child.tick(context);
        BehaviorStatus::Success
    }
    
    fn reset(&mut self) {
        self.child.reset();
    }
}

/// Condition node - wraps a condition function
pub struct Condition<F>
where
    F: Fn(&dyn BehaviorContext) -> bool + Send + Sync,
{
    condition: F,
}

impl<F> Condition<F>
where
    F: Fn(&dyn BehaviorContext) -> bool + Send + Sync,
{
    pub fn new(condition: F) -> Self {
        Self { condition }
    }
}

impl<F> BehaviorNode for Condition<F>
where
    F: Fn(&dyn BehaviorContext) -> bool + Send + Sync,
{
    fn tick(&mut self, context: &mut dyn BehaviorContext) -> BehaviorStatus {
        if (self.condition)(context) {
            BehaviorStatus::Success
        } else {
            BehaviorStatus::Failure
        }
    }
    
    fn reset(&mut self) {
        // Conditions are stateless
    }
}

/// Action node - wraps an action function
pub struct Action<F>
where
    F: FnMut(&mut dyn BehaviorContext) -> BehaviorStatus + Send + Sync,
{
    action: F,
}

impl<F> Action<F>
where
    F: FnMut(&mut dyn BehaviorContext) -> BehaviorStatus + Send + Sync,
{
    pub fn new(action: F) -> Self {
        Self { action }
    }
}

impl<F> BehaviorNode for Action<F>
where
    F: FnMut(&mut dyn BehaviorContext) -> BehaviorStatus + Send + Sync,
{
    fn tick(&mut self, context: &mut dyn BehaviorContext) -> BehaviorStatus {
        (self.action)(context)
    }
    
    fn reset(&mut self) {
        // Actions are typically stateless, but could store state if needed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Simple test context
    struct TestContext {
        value: i32,
    }
    
    impl BehaviorContext for TestContext {
        fn update(&mut self) {
            // Nothing to update
        }
    }
    
    #[test]
    fn test_selector_success() {
        let mut selector = Selector::new(vec![
            Box::new(Condition::new(|_| false)),
            Box::new(Condition::new(|_| true)),
            Box::new(Condition::new(|_| false)),
        ]);
        
        let mut context = TestContext { value: 0 };
        let status = selector.tick(&mut context);
        assert_eq!(status, BehaviorStatus::Success);
    }
    
    #[test]
    fn test_selector_failure() {
        let mut selector = Selector::new(vec![
            Box::new(Condition::new(|_| false)),
            Box::new(Condition::new(|_| false)),
            Box::new(Condition::new(|_| false)),
        ]);
        
        let mut context = TestContext { value: 0 };
        let status = selector.tick(&mut context);
        assert_eq!(status, BehaviorStatus::Failure);
    }
    
    #[test]
    fn test_sequence_success() {
        let mut sequence = Sequence::new(vec![
            Box::new(Condition::new(|_| true)),
            Box::new(Condition::new(|_| true)),
            Box::new(Condition::new(|_| true)),
        ]);
        
        let mut context = TestContext { value: 0 };
        let status = sequence.tick(&mut context);
        assert_eq!(status, BehaviorStatus::Success);
    }
    
    #[test]
    fn test_sequence_failure() {
        let mut sequence = Sequence::new(vec![
            Box::new(Condition::new(|_| true)),
            Box::new(Condition::new(|_| false)),
            Box::new(Condition::new(|_| true)),
        ]);
        
        let mut context = TestContext { value: 0 };
        let status = sequence.tick(&mut context);
        assert_eq!(status, BehaviorStatus::Failure);
    }
    
    #[test]
    fn test_inverter() {
        let mut inverter = Inverter::new(Box::new(Condition::new(|_| true)));
        let mut context = TestContext { value: 0 };
        assert_eq!(inverter.tick(&mut context), BehaviorStatus::Failure);
        
        let mut inverter = Inverter::new(Box::new(Condition::new(|_| false)));
        assert_eq!(inverter.tick(&mut context), BehaviorStatus::Success);
    }
    
    #[test]
    fn test_action() {
        let mut action = Action::new(|ctx: &mut dyn BehaviorContext| {
            let test_ctx = ctx.downcast_mut::<TestContext>().unwrap();
            test_ctx.value += 1;
            BehaviorStatus::Success
        });
        
        let mut context = TestContext { value: 0 };
        assert_eq!(action.tick(&mut context), BehaviorStatus::Success);
        assert_eq!(context.value, 1);
    }
    
    #[test]
    fn test_repeater() {
        let mut repeater = Repeater::new(
            Box::new(Action::new(|ctx: &mut dyn BehaviorContext| {
                let test_ctx = ctx.downcast_mut::<TestContext>().unwrap();
                test_ctx.value += 1;
                BehaviorStatus::Success
            })),
            3,
        );
        
        let mut context = TestContext { value: 0 };
        let status = repeater.tick(&mut context);
        assert_eq!(status, BehaviorStatus::Success);
        assert_eq!(context.value, 3);
    }
    
    #[test]
    fn test_complex_tree() {
        // Create a tree: Sequence(Condition, Selector(Condition, Condition))
        let mut tree = Sequence::new(vec![
            Box::new(Condition::new(|_| true)),
            Box::new(Selector::new(vec![
                Box::new(Condition::new(|_| false)),
                Box::new(Condition::new(|_| true)),
            ])),
        ]);
        
        let mut context = TestContext { value: 0 };
        let status = tree.tick(&mut context);
        assert_eq!(status, BehaviorStatus::Success);
    }
}

// Helper trait for downcasting BehaviorContext
trait DowncastableContext: BehaviorContext {
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl<T: BehaviorContext + 'static> DowncastableContext for T {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Extension trait for downcasting BehaviorContext
pub trait BehaviorContextExt {
    fn downcast_ref<T: 'static>(&self) -> Option<&T>;
    fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T>;
}

impl BehaviorContextExt for dyn BehaviorContext + '_ {
    fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        (self as &dyn std::any::Any).downcast_ref::<T>()
    }
    
    fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        (self as &mut dyn std::any::Any).downcast_mut::<T>()
    }
}
