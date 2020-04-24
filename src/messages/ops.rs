use crate::messages::messages::Message;

#[derive(Debug, Clone, PartialEq)]
pub enum OpType {
    OpMessage(Message),
    OpDisconnect,
    OpPiece(u32, Vec<u8>),    // idx * payload
    OpRequest(u32, u32, u32), // idx * offset * len
    OpDownStop,
    OpStop,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Op {
    pub id: u64,
    pub op_type: OpType,
}
