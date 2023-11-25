// Copyright 2021 Datafuse Labs
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::sync::atomic::AtomicU64;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;

#[derive(Default)]
pub struct Profile {
    /// The id of processor
    pub pid: usize,
    /// The name of processor
    pub p_name: String,

    pub plan_id: Option<u32>,
    pub plan_name: Option<String>,
    pub plan_parent_id: Option<u32>,

    /// The time spent to process in nanoseconds
    pub cpu_time: AtomicU64,
    /// The time spent to wait in nanoseconds, usually used to
    /// measure the time spent on waiting for I/O
    pub wait_time: AtomicU64,
}

impl Profile {
    pub fn create(pid: usize, p_name: String, scope: Option<PlanScope>) -> Profile {
        Profile {
            pid,
            p_name,
            cpu_time: AtomicU64::new(0),
            wait_time: AtomicU64::new(0),
            plan_id: scope.as_ref().map(|x| x.id),
            plan_name: scope.as_ref().map(|x| x.name.clone()),
            plan_parent_id: scope.as_ref().map(|x| x.parent_id),
        }
    }
}

pub struct PlanScopeGuard {
    idx: usize,
    scope_size: Arc<AtomicUsize>,
}

impl PlanScopeGuard {
    pub fn create(scope_size: Arc<AtomicUsize>, idx: usize) -> PlanScopeGuard {
        PlanScopeGuard { idx, scope_size }
    }
}

impl Drop for PlanScopeGuard {
    fn drop(&mut self) {
        if self.scope_size.fetch_sub(1, Ordering::SeqCst) != self.idx + 1
            && !std::thread::panicking()
        {
            panic!("Broken pipeline scope stack.");
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlanScope {
    pub id: u32,
    pub name: String,
    pub parent_id: u32,
}

impl PlanScope {
    pub fn create(id: u32, name: String) -> PlanScope {
        PlanScope {
            id,
            parent_id: 0,
            name,
        }
    }
}
