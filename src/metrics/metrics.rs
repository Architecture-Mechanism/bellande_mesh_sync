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

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct Counter {
    value: Arc<AtomicI64>,
}

impl Counter {
    fn new() -> Self {
        Self {
            value: Arc::new(AtomicI64::new(0)),
        }
    }

    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get(&self) -> i64 {
        self.value.load(Ordering::Relaxed)
    }
}

#[derive(Clone)]
pub struct Gauge {
    value: Arc<AtomicI64>,
}

impl Gauge {
    fn new() -> Self {
        Self {
            value: Arc::new(AtomicI64::new(0)),
        }
    }

    pub fn set(&self, v: i64) {
        self.value.store(v, Ordering::Relaxed);
    }

    pub fn get(&self) -> i64 {
        self.value.load(Ordering::Relaxed)
    }
}

pub struct MetricsManager {
    counters: HashMap<String, Counter>,
    gauges: HashMap<String, Gauge>,
}

impl MetricsManager {
    pub fn new() -> Self {
        let mut manager = Self {
            counters: HashMap::new(),
            gauges: HashMap::new(),
        };

        // Initialize metrics
        manager.create_gauge("active_nodes");
        manager.create_gauge("data_chunks");
        manager.create_counter("sync_operations");
        manager.create_counter("conflicts_resolved");
        manager.create_counter("network_errors");

        manager
    }

    fn create_counter(&mut self, name: &str) {
        self.counters.insert(name.to_string(), Counter::new());
    }

    fn create_gauge(&mut self, name: &str) {
        self.gauges.insert(name.to_string(), Gauge::new());
    }

    pub fn update_active_nodes(&self, count: i64) {
        if let Some(gauge) = self.gauges.get("active_nodes") {
            gauge.set(count);
        }
    }

    pub fn update_data_chunks(&self, count: i64) {
        if let Some(gauge) = self.gauges.get("data_chunks") {
            gauge.set(count);
        }
    }

    pub fn increment_sync_operations(&self) {
        if let Some(counter) = self.counters.get("sync_operations") {
            counter.inc();
        }
    }

    pub fn increment_conflicts_resolved(&self) {
        if let Some(counter) = self.counters.get("conflicts_resolved") {
            counter.inc();
        }
    }

    pub fn increment_network_errors(&self) {
        if let Some(counter) = self.counters.get("network_errors") {
            counter.inc();
        }
    }

    pub fn get_metrics(&self) -> String {
        let mut result = String::new();

        for (name, gauge) in &self.gauges {
            result.push_str(&format!("{} {}\n", name, gauge.get()));
        }

        for (name, counter) in &self.counters {
            result.push_str(&format!("{} {}\n", name, counter.get()));
        }

        result
    }
}

impl Default for MetricsManager {
    fn default() -> Self {
        Self::new()
    }
}
