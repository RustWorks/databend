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

use common_ast::ast::CreateConnectionStmt;
use common_ast::ast::UriLocation;
use common_exception::Result;

use crate::binder::parse_uri_location;
use crate::plans::CreateConnectionPlan;
use crate::plans::Plan;
use crate::Binder;

impl Binder {
    #[async_backtrace::framed]
    pub(in crate::planner::binder) async fn bind_create_connection(
        &mut self,
        stmt: &CreateConnectionStmt,
    ) -> Result<Plan> {
        let mut location = UriLocation::new(
            stmt.storage_type.clone(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            stmt.storage_params.clone(),
        );
        parse_uri_location(&mut location, None).await?;
        Ok(Plan::CreateConnection(Box::new(CreateConnectionPlan {
            if_not_exists: stmt.if_not_exists,
            name: stmt.name.to_string(),
            storage_type: stmt.storage_type.clone(),
            storage_params: stmt.storage_params.clone(),
        })))
    }
}
