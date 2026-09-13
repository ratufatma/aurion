use crate::lifecycle::fault::NodeFault;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeState {
    Starting,
    Recovering,
    Running,
    Stopped,
    Failed(NodeFault),
}

impl NodeState {
    #[inline]
    pub fn is_operational(&self) -> bool {
        matches!(self, Self::Running)
    }

    #[inline]
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}
