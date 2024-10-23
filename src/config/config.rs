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
use std::fs;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub db_url: String,
    pub listen_address: String,
    pub bootstrap_nodes: Vec<String>,
    pub sync_interval: u64,
    pub node_timeout: u64,
}

impl Config {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config_str = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&config_str)?)
    }
    pub fn sync_interval(&self) -> Option<u64> {
        Some(self.sync_interval)
    }

    pub fn node_timeout(&self) -> Option<u64> {
        Some(self.node_timeout)
    }
}
