use crate::messages::messages::Message;

#[derive(Debug, Clone, PartialEq)]
pub enum OpType {
    OpMessage(Message),
    OpPiece(u32, Vec<u8>),
    OpRequest(u32, u32, u32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Op {
    pub id: u64,
    pub op_type: OpType,
}
