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

use common_catalog::plan::PushDownInfo;
use common_catalog::table::Table;
use common_catalog::table_context::TableContext;
use common_cloud_control::client_config::build_client_config;
use common_cloud_control::cloud_api::CloudControlApiProvider;
use common_cloud_control::pb::ShowTasksRequest;
use common_cloud_control::pb::Task;
use common_cloud_control::task_client::make_request;
use common_config::GlobalConfig;
use common_exception::ErrorCode;
use common_exception::Result;
use common_expression::infer_table_schema;
use common_expression::types::StringType;
use common_expression::types::TimestampType;
use common_expression::types::UInt64Type;
use common_expression::DataBlock;
use common_expression::FromData;
use common_meta_app::schema::TableIdent;
use common_meta_app::schema::TableInfo;
use common_meta_app::schema::TableMeta;
use common_sql::plans::task_schema;

use crate::table::AsyncOneBlockSystemTable;
use crate::table::AsyncSystemTable;

pub fn parse_tasks_to_datablock(tasks: Vec<Task>) -> Result<DataBlock> {
    let mut created_on: Vec<i64> = Vec::with_capacity(tasks.len());
    let mut name: Vec<Vec<u8>> = Vec::with_capacity(tasks.len());
    let mut id: Vec<u64> = Vec::with_capacity(tasks.len());
    let mut owner: Vec<Vec<u8>> = Vec::with_capacity(tasks.len());
    let mut comment: Vec<Option<Vec<u8>>> = Vec::with_capacity(tasks.len());
    let mut warehouse: Vec<Option<Vec<u8>>> = Vec::with_capacity(tasks.len());
    let mut schedule: Vec<Option<Vec<u8>>> = Vec::with_capacity(tasks.len());
    let mut status: Vec<Vec<u8>> = Vec::with_capacity(tasks.len());
    let mut definition: Vec<Vec<u8>> = Vec::with_capacity(tasks.len());
    let mut suspend_after_num_failures: Vec<Option<u64>> = Vec::with_capacity(tasks.len());
    let mut last_committed_on: Vec<i64> = Vec::with_capacity(tasks.len());
    let mut next_schedule_time: Vec<Option<i64>> = Vec::with_capacity(tasks.len());
    let mut last_suspended_on: Vec<Option<i64>> = Vec::with_capacity(tasks.len());

    for task in tasks {
        let tsk: common_cloud_control::task_utils::Task = task.try_into()?;
        created_on.push(tsk.created_at.timestamp_micros());
        name.push(tsk.task_name.into_bytes());
        id.push(tsk.task_id);
        owner.push(tsk.owner.into_bytes());
        comment.push(tsk.comment.map(|s| s.into_bytes()));
        warehouse.push(
            tsk.warehouse_options
                .and_then(|s| s.warehouse.map(|v| v.into_bytes())),
        );
        schedule.push(tsk.schedule_options.map(|s| s.into_bytes()));
        status.push(tsk.status.to_string().into_bytes());
        definition.push(tsk.query_text.into_bytes());
        suspend_after_num_failures.push(tsk.suspend_task_after_num_failures.map(|v| v as u64));
        next_schedule_time.push(tsk.next_scheduled_at.map(|t| t.timestamp_micros()));
        last_committed_on.push(tsk.updated_at.timestamp_micros());
        last_suspended_on.push(tsk.last_suspended_at.map(|t| t.timestamp_micros()));
    }
    Ok(DataBlock::new_from_columns(vec![
        TimestampType::from_data(created_on),
        StringType::from_data(name),
        UInt64Type::from_data(id),
        StringType::from_data(owner),
        StringType::from_opt_data(comment),
        StringType::from_opt_data(warehouse),
        StringType::from_opt_data(schedule),
        StringType::from_data(status),
        StringType::from_data(definition),
        UInt64Type::from_opt_data(suspend_after_num_failures),
        TimestampType::from_opt_data(next_schedule_time),
        TimestampType::from_data(last_committed_on),
        TimestampType::from_opt_data(last_suspended_on),
    ]))
}

pub struct TasksTable {
    table_info: TableInfo,
}

#[async_trait::async_trait]
impl AsyncSystemTable for TasksTable {
    const NAME: &'static str = "system.tasks";

    fn get_table_info(&self) -> &TableInfo {
        &self.table_info
    }

    #[async_backtrace::framed]
    async fn get_full_data(
        &self,
        ctx: Arc<dyn TableContext>,
        _push_downs: Option<PushDownInfo>,
    ) -> Result<DataBlock> {
        let config = GlobalConfig::instance();
        if config.query.cloud_control_grpc_server_address.is_none() {
            return Err(ErrorCode::CloudControlNotEnabled(
                "cannot view system.tasks table without cloud control enabled, please set cloud_control_grpc_server_address in config",
            ));
        }

        let tenant = ctx.get_tenant();
        let query_id = ctx.get_id();
        let user = ctx.get_current_user()?.identity().to_string();
        let available_roles = ctx.get_available_roles().await?;
        let req = ShowTasksRequest {
            tenant_id: tenant.clone(),
            name_like: "".to_string(),
            result_limit: 10000, // TODO: use plan.limit pushdown
            owners: available_roles
                .into_iter()
                .map(|x| x.identity().to_string())
                .collect(),
            task_ids: vec![],
        };

        let cloud_api = CloudControlApiProvider::instance();
        let task_client = cloud_api.get_task_client();
        let config = build_client_config(tenant, user, query_id);
        let req = make_request(req, config);

        let resp = task_client.show_tasks(req).await?;
        let tasks = resp.tasks;

        parse_tasks_to_datablock(tasks)
    }
}

impl TasksTable {
    pub fn create(table_id: u64) -> Arc<dyn Table> {
        let schema = infer_table_schema(&task_schema()).expect("failed to parse task table schema");

        let table_info = TableInfo {
            desc: "'system'.'tasks'".to_string(),
            name: "tasks".to_string(),
            ident: TableIdent::new(table_id, 0),
            meta: TableMeta {
                schema,
                engine: "SystemTasks".to_string(),

                ..Default::default()
            },
            ..Default::default()
        };

        AsyncOneBlockSystemTable::create(Self { table_info })
    }
}
