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

use common_ast::ast::AlterTaskOptions;
use common_ast::ast::ScheduleOptions;
use common_ast::ast::ShowLimit;
use common_ast::ast::WarehouseOptions;
use common_expression::types::DataType;
use common_expression::types::NumberDataType::Int32;
use common_expression::types::NumberDataType::Int64;
use common_expression::types::NumberDataType::UInt64;
use common_expression::DataField;
use common_expression::DataSchema;
use common_expression::DataSchemaRef;
use common_expression::DataSchemaRefExt;

pub fn task_schema() -> DataSchemaRef {
    Arc::new(DataSchema::new(vec![
        DataField::new("created_on", DataType::Timestamp),
        DataField::new("name", DataType::String),
        DataField::new("id", DataType::Number(UInt64)),
        DataField::new("owner", DataType::String),
        DataField::new("comment", DataType::String.wrap_nullable()),
        DataField::new("warehouse", DataType::String.wrap_nullable()),
        DataField::new("schedule", DataType::String.wrap_nullable()),
        DataField::new("state", DataType::String),
        DataField::new("definition", DataType::String),
        DataField::new(
            "suspend_task_after_num_failures",
            DataType::Number(UInt64).wrap_nullable(),
        ),
        DataField::new("next_schedule_time", DataType::Timestamp.wrap_nullable()),
        DataField::new("last_committed_on", DataType::Timestamp),
        DataField::new("last_suspended_on", DataType::Timestamp.wrap_nullable()),
    ]))
}

pub fn task_run_schema() -> DataSchemaRef {
    Arc::new(DataSchema::new(vec![
        DataField::new("name", DataType::String),
        DataField::new("id", DataType::Number(UInt64)),
        DataField::new("owner", DataType::String),
        DataField::new("comment", DataType::String.wrap_nullable()),
        DataField::new("schedule", DataType::String.wrap_nullable()),
        DataField::new("warehouse", DataType::String.wrap_nullable()),
        DataField::new("state", DataType::String),
        DataField::new("definition", DataType::String),
        DataField::new("run_id", DataType::String),
        DataField::new("query_id", DataType::String),
        DataField::new("exception_code", DataType::Number(Int64)),
        DataField::new("exception_text", DataType::String.wrap_nullable()),
        DataField::new("attempt_number", DataType::Number(Int32)),
        DataField::new("completed_time", DataType::Timestamp.wrap_nullable()),
        DataField::new("scheduled_time", DataType::Timestamp),
    ]))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateTaskPlan {
    pub if_not_exists: bool,
    pub tenant: String,
    pub task_name: String,
    pub warehouse_opts: WarehouseOptions,
    pub schedule_opts: ScheduleOptions,
    pub suspend_task_after_num_failures: Option<u64>,
    pub sql: String,
    pub comment: String,
}

impl CreateTaskPlan {
    pub fn schema(&self) -> DataSchemaRef {
        DataSchemaRefExt::create(vec![])
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlterTaskPlan {
    pub if_exists: bool,
    pub tenant: String,
    pub task_name: String,
    pub alter_options: AlterTaskOptions,
}

impl AlterTaskPlan {
    pub fn schema(&self) -> DataSchemaRef {
        DataSchemaRefExt::create(vec![])
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropTaskPlan {
    pub if_exists: bool,
    pub tenant: String,
    pub task_name: String,
}

impl DropTaskPlan {
    pub fn schema(&self) -> DataSchemaRef {
        DataSchemaRefExt::create(vec![])
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DescribeTaskPlan {
    pub tenant: String,
    pub task_name: String,
}

impl DescribeTaskPlan {
    pub fn schema(&self) -> DataSchemaRef {
        task_schema()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecuteTaskPlan {
    pub tenant: String,
    pub task_name: String,
}

impl ExecuteTaskPlan {
    pub fn schema(&self) -> DataSchemaRef {
        DataSchemaRefExt::create(vec![])
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShowTasksPlan {
    pub tenant: String,
    pub limit: Option<ShowLimit>,
}

impl ShowTasksPlan {
    pub fn schema(&self) -> DataSchemaRef {
        task_schema()
    }
}
