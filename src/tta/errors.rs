use std::{error::Error, fmt};

#[derive(Debug)]
pub enum TtaError {
    DatabaseError(sqlx::Error),
}

// implementation of Display trait
impl fmt::Display for TtaError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TtaError::DatabaseError(e) => write!(f, "Database error: {}", e),
        }
    }
}

// implementation of Error trait
impl Error for TtaError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            TtaError::DatabaseError(e) => Some(e),
        }
    }
}
