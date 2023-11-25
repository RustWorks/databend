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
use common_expression::types::StringType;
use common_expression::DataBlock;
use common_expression::FromData;
use common_sql::plans::DescConnectionPlan;
use common_users::UserApiProvider;

use crate::interpreters::Interpreter;
use crate::pipelines::PipelineBuildResult;
use crate::sessions::QueryContext;
use crate::sessions::TableContext;

#[derive(Debug)]
pub struct DescConnectionInterpreter {
    ctx: Arc<QueryContext>,
    plan: DescConnectionPlan,
}

impl DescConnectionInterpreter {
    pub fn try_create(ctx: Arc<QueryContext>, plan: DescConnectionPlan) -> Result<Self> {
        Ok(DescConnectionInterpreter { ctx, plan })
    }
}

#[async_trait::async_trait]
impl Interpreter for DescConnectionInterpreter {
    fn name(&self) -> &str {
        "DescConnectionInterpreter"
    }

    #[async_backtrace::framed]
    async fn execute2(&self) -> Result<PipelineBuildResult> {
        let tenant = self.ctx.get_tenant();
        let user_mgr = UserApiProvider::instance();

        let connection = user_mgr
            .get_connection(&tenant, self.plan.name.as_str())
            .await?;

        let names = vec![connection.name.as_bytes().to_vec()];
        let types = vec![connection.storage_type.as_bytes().to_vec()];
        let params = vec![connection.storage_params_display().as_bytes().to_vec()];

        PipelineBuildResult::from_blocks(vec![DataBlock::new_from_columns(vec![
            StringType::from_data(names),
            StringType::from_data(types),
            StringType::from_data(params),
        ])])
    }
}
