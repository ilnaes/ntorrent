use crate::messages::messages::Message;

#[derive(Debug, Clone, PartialEq)]
pub enum OpID {
    Message,
    Exit,
}

#[derive(Debug, Clone)]
pub struct Op {
    pub op_id: OpID,
    pub payload: Option<Message>,
}
