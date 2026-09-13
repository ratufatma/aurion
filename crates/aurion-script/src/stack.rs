use crate::error::ScriptError;

pub const MAX_STACK_DEPTH: usize = 1024;
pub const MAX_ELEMENT_SIZE: usize = 520;

#[derive(Debug, Clone, Default)]
pub struct Stack {
    items: Vec<Vec<u8>>,
}

impl Stack {
    pub fn new() -> Self {
        Self {
            items: Vec::with_capacity(16),
        }
    }

    pub fn push(&mut self, item: Vec<u8>) -> Result<(), ScriptError> {
        if self.items.len() >= MAX_STACK_DEPTH {
            return Err(ScriptError::StackOverflow(MAX_STACK_DEPTH));
        }
        if item.len() > MAX_ELEMENT_SIZE {
            return Err(ScriptError::ElementTooLarge {
                size: item.len(),
                max: MAX_ELEMENT_SIZE,
            });
        }
        self.items.push(item);
        Ok(())
    }

    pub fn pop(&mut self) -> Result<Vec<u8>, ScriptError> {
        self.items.pop().ok_or(ScriptError::StackUnderflow)
    }

    pub fn peek(&self) -> Result<&[u8], ScriptError> {
        self.items
            .last()
            .map(|v| v.as_slice())
            .ok_or(ScriptError::StackUnderflow)
    }

    pub fn dup(&mut self) -> Result<(), ScriptError> {
        let top = self.peek()?.to_vec();
        self.push(top)
    }

    pub fn drop(&mut self) -> Result<(), ScriptError> {
        let _ = self.pop()?;
        Ok(())
    }

    pub fn swap(&mut self) -> Result<(), ScriptError> {
        let len = self.items.len();
        if len < 2 {
            return Err(ScriptError::StackUnderflow);
        }
        let idx1 = len.saturating_sub(1);
        let idx2 = len.saturating_sub(2);
        self.items.swap(idx1, idx2);
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
}
