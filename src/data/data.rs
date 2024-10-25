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

use crate::node::node::NodeId;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DataChunk {
    pub id: NodeId,
    pub content: Vec<u8>,
    pub checksum: String,
    pub version: u64,
    pub last_modified: SystemTime,
    pub author: NodeId,
    pub parent_versions: Vec<u64>,
}

impl DataChunk {
    pub fn new(
        content: Vec<u8>,
        checksum: String,
        version: u64,
        author: NodeId,
        parent_versions: Vec<u64>,
    ) -> Self {
        Self {
            id: NodeId::new(),
            content,
            checksum,
            version,
            last_modified: SystemTime::now(),
            author: NodeId::new(),
            parent_versions,
        }
    }
    pub fn size(&self) -> usize {
        std::mem::size_of::<NodeId>() * 2 + self.content.len() + std::mem::size_of::<SystemTime>()
    }
}
