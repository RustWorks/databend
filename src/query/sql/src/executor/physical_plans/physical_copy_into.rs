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

use common_catalog::plan::DataSourcePlan;
use common_catalog::plan::StageTableInfo;
use common_exception::Result;
use common_expression::DataSchemaRef;
use common_expression::Scalar;
use common_meta_app::schema::CatalogInfo;
use common_meta_app::schema::TableInfo;
use common_storage::StageFileInfo;
use enum_as_inner::EnumAsInner;

use crate::executor::physical_plan::PhysicalPlan;
use crate::plans::CopyIntoTableMode;
use crate::plans::ValidationMode;
use crate::ColumnBinding;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CopyIntoTable {
    pub catalog_info: CatalogInfo,
    pub required_values_schema: DataSchemaRef,
    pub values_consts: Vec<Scalar>,
    pub required_source_schema: DataSchemaRef,
    pub write_mode: CopyIntoTableMode,
    pub validation_mode: ValidationMode,
    pub force: bool,
    pub stage_table_info: StageTableInfo,
    pub files: Vec<StageFileInfo>,
    pub table_info: TableInfo,

    pub source: CopyIntoTableSource,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct QuerySource {
    pub plan: PhysicalPlan,
    pub query_source_schema: DataSchemaRef,
    pub ignore_result: bool,
    pub result_columns: Vec<ColumnBinding>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, EnumAsInner)]
pub enum CopyIntoTableSource {
    Query(Box<QuerySource>),
    Stage(Box<DataSourcePlan>),
}

impl CopyIntoTable {
    pub fn output_schema(&self) -> Result<DataSchemaRef> {
        match &self.source {
            CopyIntoTableSource::Query(query_ctx) => Ok(query_ctx.query_source_schema.clone()),
            CopyIntoTableSource::Stage(_) => Ok(self.required_values_schema.clone()),
        }
    }
}
