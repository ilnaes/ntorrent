use std::error;
use std::fmt;

#[derive(Debug, Clone)]
pub struct ConnectError;

#[derive(Debug, Clone)]
pub struct CommunicationError;

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid first item to double")
    }
}

impl error::Error for ConnectError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}
