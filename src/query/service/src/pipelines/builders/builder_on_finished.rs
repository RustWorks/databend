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
use std::time::Instant;

use common_base::runtime::GlobalIORuntime;
use common_catalog::table_context::TableContext;
use common_exception::Result;
use common_meta_app::principal::StageInfo;
use common_metrics::storage::*;
use common_pipeline_core::Pipeline;
use common_storage::StageFileInfo;
use common_storages_fuse::io::Files;
use common_storages_stage::StageTable;
use log::error;

use crate::pipelines::PipelineBuilder;
use crate::sessions::QueryContext;

impl PipelineBuilder {
    pub fn set_purge_files_on_finished(
        ctx: Arc<QueryContext>,
        files: Vec<StageFileInfo>,
        copy_purge_option: bool,
        stage_info: StageInfo,
        main_pipeline: &mut Pipeline,
    ) -> Result<()> {
        // set on_finished callback.
        main_pipeline.set_on_finished(move |may_error| {
            match may_error {
                None => {
                    GlobalIORuntime::instance().block_on(async move {
                        // 1. log on_error mode errors.
                        // todo(ariesdevil): persist errors with query_id
                        if let Some(error_map) = ctx.get_maximum_error_per_file() {
                            for (file_name, e) in error_map {
                                error!(
                                    "copy(on_error={}): file {} encounter error {},",
                                    stage_info.copy_options.on_error,
                                    file_name,
                                    e.to_string()
                                );
                            }
                        }

                        // 2. Try to purge copied files if purge option is true, if error will skip.
                        // If a file is already copied(status with AlreadyCopied) we will try to purge them.
                        if copy_purge_option {
                            let start = Instant::now();
                            Self::try_purge_files(ctx.clone(), &stage_info, &files).await;

                            // Perf.
                            {
                                metrics_inc_copy_purge_files_counter(files.len() as u32);
                                metrics_inc_copy_purge_files_cost_milliseconds(
                                    start.elapsed().as_millis() as u32,
                                );
                            }
                        }

                        Ok(())
                    })?;
                }
                Some(error) => {
                    error!("copy failed, reason: {}", error);
                }
            }
            Ok(())
        });
        Ok(())
    }

    #[async_backtrace::framed]
    async fn try_purge_files(
        ctx: Arc<QueryContext>,
        stage_info: &StageInfo,
        stage_files: &[StageFileInfo],
    ) {
        let table_ctx: Arc<dyn TableContext> = ctx.clone();
        let op = StageTable::get_op(stage_info);
        match op {
            Ok(op) => {
                let file_op = Files::create(table_ctx, op);
                let files = stage_files
                    .iter()
                    .map(|v| v.path.clone())
                    .collect::<Vec<_>>();
                if let Err(e) = file_op.remove_file_in_batch(&files).await {
                    error!("Failed to delete file: {:?}, error: {}", files, e);
                }
            }
            Err(e) => {
                error!("Failed to get stage table op, error: {}", e);
            }
        }
    }
}
