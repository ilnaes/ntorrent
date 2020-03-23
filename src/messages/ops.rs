use crate::messages::messages::Message;

#[derive(Debug, Clone, PartialEq)]
pub enum OpType {
    OpMessage(Message),
    OpPiece(usize, Vec<u8>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Op {
    pub id: u64,
    pub op_type: OpType,
}
