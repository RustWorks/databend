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

use common_exception::Result;
use common_expression::types::DataType;
use common_expression::BlockEntry;
use common_expression::DataBlock;
use common_expression::Scalar;
use common_expression::Value;
use common_meta_app::schema::CatalogOption;
use common_meta_app::storage::StorageParams;
use common_sql::plans::ShowCreateCatalogPlan;
use log::debug;

use crate::interpreters::Interpreter;
use crate::pipelines::PipelineBuildResult;
use crate::sessions::QueryContext;
use crate::sessions::TableContext;

pub struct ShowCreateCatalogInterpreter {
    ctx: Arc<QueryContext>,
    plan: ShowCreateCatalogPlan,
}

impl ShowCreateCatalogInterpreter {
    pub fn try_create(ctx: Arc<QueryContext>, plan: ShowCreateCatalogPlan) -> Result<Self> {
        Ok(ShowCreateCatalogInterpreter { ctx, plan })
    }
}

#[async_trait::async_trait]
impl Interpreter for ShowCreateCatalogInterpreter {
    fn name(&self) -> &str {
        "ShowCreateTableInterpreter"
    }

    #[async_backtrace::framed]
    async fn execute2(&self) -> Result<PipelineBuildResult> {
        let catalog = self.ctx.get_catalog(self.plan.catalog.as_str()).await?;

        let name = catalog.name();
        let info = catalog.info();

        let (catalog_type, option) = match info.meta.catalog_option {
            CatalogOption::Default => (String::from("default"), String::new()),
            CatalogOption::Hive(op) => (
                String::from("hive"),
                format!(
                    "METASTORE ADDRESS\n{}\nSTORAGE PARAMS\n{}",
                    op.address,
                    op.storage_params.unwrap_or(Box::new(StorageParams::None))
                ),
            ),
            CatalogOption::Iceberg(op) => (
                String::from("iceberg"),
                format!("STORAGE PARAMS\n{}", op.storage_params),
            ),
        };

        let block = DataBlock::new(
            vec![
                BlockEntry::new(
                    DataType::String,
                    Value::Scalar(Scalar::String(name.into_bytes())),
                ),
                BlockEntry::new(
                    DataType::String,
                    Value::Scalar(Scalar::String(catalog_type.into_bytes())),
                ),
                BlockEntry::new(
                    DataType::String,
                    Value::Scalar(Scalar::String(option.into_bytes())),
                ),
            ],
            1,
        );
        debug!("Show create catalog executor result: {:?}", block);

        PipelineBuildResult::from_blocks(vec![block])
    }
}
