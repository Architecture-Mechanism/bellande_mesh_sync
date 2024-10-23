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

use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DataChunk {
    pub id: Uuid,
    pub content: Vec<u8>,
    pub checksum: String,
    pub version: u64,
    pub last_modified: SystemTime,
    pub author: Uuid,
    pub parent_versions: Vec<u64>,
}

impl DataChunk {
    pub fn new(
        content: Vec<u8>,
        checksum: String,
        version: u64,
        author: Uuid,
        parent_versions: Vec<u64>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            content,
            checksum,
            version,
            last_modified: SystemTime::now(),
            author,
            parent_versions,
        }
    }
}
