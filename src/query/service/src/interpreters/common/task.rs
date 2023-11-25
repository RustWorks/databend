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

use common_ast::ast::ScheduleOptions;
use common_ast::ast::WarehouseOptions;
use common_catalog::table_context::TableContext;
use common_cloud_control::client_config::build_client_config;
use common_cloud_control::client_config::ClientConfig;
use common_cloud_control::pb::schedule_options::ScheduleType;
use common_exception::Result;

use crate::sessions::QueryContext;

pub fn make_schedule_options(opt: ScheduleOptions) -> common_cloud_control::pb::ScheduleOptions {
    match opt {
        ScheduleOptions::IntervalMinutes(minute) => common_cloud_control::pb::ScheduleOptions {
            interval: Some(minute as i32),
            cron: None,
            time_zone: None,
            schedule_type: i32::from(ScheduleType::IntervalType),
        },
        ScheduleOptions::CronExpression(expr, timezone) => {
            common_cloud_control::pb::ScheduleOptions {
                interval: None,
                cron: Some(expr),
                time_zone: timezone,
                schedule_type: i32::from(ScheduleType::CronType),
            }
        }
    }
}

pub fn make_warehouse_options(opt: WarehouseOptions) -> common_cloud_control::pb::WarehouseOptions {
    let mut ret = common_cloud_control::pb::WarehouseOptions {
        warehouse: None,
        using_warehouse_size: None,
    };
    if let Some(warehouse) = opt.warehouse {
        ret.warehouse = Some(warehouse);
    }
    ret
}

pub fn get_client_config(ctx: Arc<QueryContext>) -> Result<ClientConfig> {
    let tenant = ctx.get_tenant();
    let user = ctx.get_current_user()?.identity().to_string();
    let query_id = ctx.get_id();

    Ok(build_client_config(tenant, user, query_id))
}
