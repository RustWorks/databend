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

use std::sync::Arc;

use common_meta_app::schema::CatalogInfo;
use common_meta_app::schema::TableInfo;
use common_meta_app::schema::UpdateStreamMetaReq;
use storages_common_table_meta::meta::TableSnapshot;

use crate::executor::physical_plans::common::MutationKind;
use crate::executor::PhysicalPlan;

// TODO(sky): make TableMutationAggregator distributed
/// The commit sink is used to commit the data to the table.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CommitSink {
    pub input: Box<PhysicalPlan>,
    pub snapshot: Arc<TableSnapshot>,
    pub table_info: TableInfo,
    pub catalog_info: CatalogInfo,
    pub mutation_kind: MutationKind,
    pub update_stream_meta: Vec<UpdateStreamMetaReq>,
    pub merge_meta: bool,
    pub need_lock: bool,
}
