// Copyright (C) 2024 Bellande Architecture Mechanism Research Innovation Center, Ronaldson Bellande

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::array::TryFromSliceError;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum BellandeMeshError {
    IoError(std::io::Error),
    LockError,
    ConversionError,
    PersistenceError(String),
    InvalidAddress,
    ProtocolError(String),
    Serialization(String),
    Database(String),
    Encryption(String),
    Authentication,
    NodeNotFound,
    Dht(String),
    RateLimitExceeded,
    ConflictResolution,
    Migration(String),
    Deserialization(String),
    NetworkError(String),
    ArrayConversionError(TryFromSliceError),
    Timeout,
}

impl fmt::Display for BellandeMeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BellandeMeshError::IoError(e) => write!(f, "IO error: {}", e),
            BellandeMeshError::LockError => write!(f, "Lock acquisition failed"),
            BellandeMeshError::ConversionError => write!(f, "Type conversion error"),
            BellandeMeshError::PersistenceError(e) => write!(f, "Persistence error: {}", e),
            BellandeMeshError::InvalidAddress => write!(f, "Invalid address"),
            BellandeMeshError::ProtocolError(e) => write!(f, "Protocol error: {}", e),
            BellandeMeshError::Serialization(err) => write!(f, "Serialization error: {}", err),
            BellandeMeshError::Database(err) => write!(f, "Database error: {}", err),
            BellandeMeshError::Encryption(err) => write!(f, "Encryption error: {}", err),
            BellandeMeshError::Authentication => write!(f, "Authentication error"),
            BellandeMeshError::NodeNotFound => write!(f, "Node not found"),
            BellandeMeshError::Dht(err) => write!(f, "DHT error: {}", err),
            BellandeMeshError::RateLimitExceeded => write!(f, "Rate limit exceeded"),
            BellandeMeshError::ConflictResolution => write!(f, "Conflict resolution error"),
            BellandeMeshError::Migration(err) => write!(f, "Migration error: {}", err),
            BellandeMeshError::Deserialization(err) => write!(f, "Deserialization error: {}", err),
            BellandeMeshError::NetworkError(e) => write!(f, "Network error: {}", e),
            BellandeMeshError::ArrayConversionError(e) => {
                write!(f, "Array conversion error: {}", e)
            }
            BellandeMeshError::Timeout => write!(f, "Operation timed out"),
        }
    }
}

impl Error for BellandeMeshError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            BellandeMeshError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for BellandeMeshError {
    fn from(err: std::io::Error) -> Self {
        BellandeMeshError::IoError(err)
    }
}
