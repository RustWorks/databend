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

use common_base::base::tokio::sync::Barrier;
use common_exception::Result;
use common_pipeline_core::processors::InputPort;
use common_pipeline_core::processors::ProcessorPtr;
use common_pipeline_core::Pipe;
use common_pipeline_core::PipeItem;
use common_pipeline_sinks::Sinker;
use common_pipeline_transforms::processors::ProcessorProfileWrapper;
use common_pipeline_transforms::processors::ProfileStub;
use common_pipeline_transforms::processors::Transformer;
use common_sql::executor::physical_plans::HashJoin;
use common_sql::executor::physical_plans::MaterializedCte;
use common_sql::executor::physical_plans::RangeJoin;
use common_sql::executor::physical_plans::RuntimeFilterSource;
use common_sql::executor::PhysicalPlan;
use common_sql::ColumnBinding;
use common_sql::IndexType;

use crate::pipelines::processors::transforms::range_join::RangeJoinState;
use crate::pipelines::processors::transforms::range_join::TransformRangeJoinLeft;
use crate::pipelines::processors::transforms::range_join::TransformRangeJoinRight;
use crate::pipelines::processors::transforms::BuildSpillCoordinator;
use crate::pipelines::processors::transforms::BuildSpillState;
use crate::pipelines::processors::transforms::HashJoinBuildState;
use crate::pipelines::processors::transforms::HashJoinProbeState;
use crate::pipelines::processors::transforms::MaterializedCteSink;
use crate::pipelines::processors::transforms::MaterializedCteState;
use crate::pipelines::processors::transforms::ProbeSpillState;
use crate::pipelines::processors::transforms::RuntimeFilterState;
use crate::pipelines::processors::transforms::TransformHashJoinBuild;
use crate::pipelines::processors::transforms::TransformHashJoinProbe;
use crate::pipelines::processors::HashJoinDesc;
use crate::pipelines::processors::HashJoinState;
use crate::pipelines::processors::SinkRuntimeFilterSource;
use crate::pipelines::processors::TransformRuntimeFilter;
use crate::pipelines::PipelineBuilder;
use crate::sessions::QueryContext;

impl PipelineBuilder {
    pub(crate) fn build_range_join(&mut self, range_join: &RangeJoin) -> Result<()> {
        let state = Arc::new(RangeJoinState::new(self.ctx.clone(), range_join));
        self.expand_right_side_pipeline(range_join, state.clone())?;
        self.build_left_side(range_join, state)?;
        if self.enable_profiling {
            self.main_pipeline.add_transform(|input, output| {
                Ok(ProcessorPtr::create(Transformer::create(
                    input,
                    output,
                    ProfileStub::new(range_join.plan_id, self.proc_profs.clone())
                        .accumulate_output_rows()
                        .accumulate_output_bytes(),
                )))
            })?;
        }
        Ok(())
    }

    fn build_left_side(
        &mut self,
        range_join: &RangeJoin,
        state: Arc<RangeJoinState>,
    ) -> Result<()> {
        self.build_pipeline(&range_join.left)?;
        let max_threads = self.settings.get_max_threads()? as usize;
        self.main_pipeline.try_resize(max_threads)?;
        self.main_pipeline.add_transform(|input, output| {
            let transform = TransformRangeJoinLeft::create(input, output, state.clone());
            if self.enable_profiling {
                Ok(ProcessorPtr::create(ProcessorProfileWrapper::create(
                    transform,
                    range_join.plan_id,
                    self.proc_profs.clone(),
                )))
            } else {
                Ok(ProcessorPtr::create(transform))
            }
        })?;
        Ok(())
    }

    fn expand_right_side_pipeline(
        &mut self,
        range_join: &RangeJoin,
        state: Arc<RangeJoinState>,
    ) -> Result<()> {
        let right_side_context = QueryContext::create_from(self.ctx.clone());
        let mut right_side_builder = PipelineBuilder::create(
            self.func_ctx.clone(),
            self.settings.clone(),
            right_side_context,
            self.enable_profiling,
            self.proc_profs.clone(),
            self.main_pipeline.plans_scope.clone(),
        );
        right_side_builder.cte_state = self.cte_state.clone();
        let mut right_res = right_side_builder.finalize(&range_join.right)?;
        right_res.main_pipeline.add_sink(|input| {
            let transform = Sinker::<TransformRangeJoinRight>::create(
                input,
                TransformRangeJoinRight::create(state.clone()),
            );
            if self.enable_profiling {
                Ok(ProcessorPtr::create(ProcessorProfileWrapper::create(
                    transform,
                    range_join.plan_id,
                    self.proc_profs.clone(),
                )))
            } else {
                Ok(ProcessorPtr::create(transform))
            }
        })?;
        self.pipelines.push(right_res.main_pipeline.finalize());
        self.pipelines.extend(right_res.sources_pipelines);
        Ok(())
    }

    pub(crate) fn build_join(&mut self, join: &HashJoin) -> Result<()> {
        let state = self.build_join_state(join)?;
        self.expand_build_side_pipeline(&join.build, join, state.clone())?;
        self.build_join_probe(join, state)
    }

    fn build_join_state(&mut self, join: &HashJoin) -> Result<Arc<HashJoinState>> {
        HashJoinState::try_create(
            self.ctx.clone(),
            join.build.output_schema()?,
            &join.build_projections,
            HashJoinDesc::create(join)?,
            &join.probe_to_build,
        )
    }

    fn expand_build_side_pipeline(
        &mut self,
        build: &PhysicalPlan,
        hash_join_plan: &HashJoin,
        join_state: Arc<HashJoinState>,
    ) -> Result<()> {
        let build_side_context = QueryContext::create_from(self.ctx.clone());
        let mut build_side_builder = PipelineBuilder::create(
            self.func_ctx.clone(),
            self.settings.clone(),
            build_side_context,
            self.enable_profiling,
            self.proc_profs.clone(),
            self.main_pipeline.plans_scope.clone(),
        );
        build_side_builder.cte_state = self.cte_state.clone();
        let mut build_res = build_side_builder.finalize(build)?;

        assert!(build_res.main_pipeline.is_pulling_pipeline()?);
        let output_len = build_res.main_pipeline.output_len();
        let spill_coordinator = BuildSpillCoordinator::create(output_len);
        let barrier = Barrier::new(output_len);
        let restore_barrier = Barrier::new(output_len);
        let build_state = HashJoinBuildState::try_create(
            self.ctx.clone(),
            self.func_ctx.clone(),
            &hash_join_plan.build_keys,
            &hash_join_plan.build_projections,
            join_state,
            barrier,
            restore_barrier,
        )?;

        let create_sink_processor = |input| {
            let spill_state = if self.settings.get_join_spilling_threshold()? != 0 {
                Some(Box::new(BuildSpillState::create(
                    self.ctx.clone(),
                    spill_coordinator.clone(),
                    build_state.clone(),
                )))
            } else {
                None
            };
            let transform =
                TransformHashJoinBuild::try_create(input, build_state.clone(), spill_state)?;

            if self.enable_profiling {
                Ok(ProcessorPtr::create(ProcessorProfileWrapper::create(
                    transform,
                    hash_join_plan.plan_id,
                    self.proc_profs.clone(),
                )))
            } else {
                Ok(ProcessorPtr::create(transform))
            }
        };
        if hash_join_plan.contain_runtime_filter {
            build_res.main_pipeline.duplicate(false)?;
            self.join_state = Some(build_state.clone());
            self.index = Some(self.pipelines.len());
        } else {
            // for merge into
            if hash_join_plan.need_hold_hash_table {
                self.join_state = Some(build_state.clone())
            }
            build_res.main_pipeline.add_sink(create_sink_processor)?;
        }

        self.pipelines.push(build_res.main_pipeline.finalize());
        self.pipelines.extend(build_res.sources_pipelines);
        Ok(())
    }

    fn build_join_probe(&mut self, join: &HashJoin, state: Arc<HashJoinState>) -> Result<()> {
        self.build_pipeline(&join.probe)?;

        let max_block_size = self.settings.get_max_block_size()? as usize;
        let barrier = Barrier::new(self.main_pipeline.output_len());
        let restore_barrier = Barrier::new(self.main_pipeline.output_len());
        let probe_state = Arc::new(HashJoinProbeState::create(
            self.ctx.clone(),
            self.func_ctx.clone(),
            state,
            &join.probe_projections,
            &join.probe_keys,
            join.probe.output_schema()?,
            &join.join_type,
            self.main_pipeline.output_len(),
            barrier,
            restore_barrier,
        )?);
        let mut has_string_column = false;
        for filed in join.output_schema()?.fields() {
            has_string_column |= filed.data_type().is_string_column();
        }

        self.main_pipeline.add_transform(|input, output| {
            let probe_spill_state = if self.settings.get_join_spilling_threshold()? != 0 {
                Some(Box::new(ProbeSpillState::create(
                    self.ctx.clone(),
                    probe_state.clone(),
                )))
            } else {
                None
            };
            let transform = TransformHashJoinProbe::create(
                input,
                output,
                join.projections.clone(),
                probe_state.clone(),
                probe_spill_state,
                max_block_size,
                self.func_ctx.clone(),
                &join.join_type,
                !join.non_equi_conditions.is_empty(),
                has_string_column,
            )?;

            if self.enable_profiling {
                Ok(ProcessorPtr::create(ProcessorProfileWrapper::create(
                    transform,
                    join.plan_id,
                    self.proc_profs.clone(),
                )))
            } else {
                Ok(ProcessorPtr::create(transform))
            }
        })?;

        if self.enable_profiling {
            // Add a stub after the probe processor to accumulate the output rows.
            self.main_pipeline.add_transform(|input, output| {
                Ok(ProcessorPtr::create(Transformer::create(
                    input,
                    output,
                    ProfileStub::new(join.plan_id, self.proc_profs.clone())
                        .accumulate_output_rows()
                        .accumulate_output_bytes(),
                )))
            })?;
        }

        if join.need_hold_hash_table {
            let mut projected_probe_fields = vec![];
            for (i, field) in probe_state.probe_schema.fields().iter().enumerate() {
                if probe_state.probe_projections.contains(&i) {
                    projected_probe_fields.push(field.clone());
                }
            }
            self.probe_data_fields = Some(projected_probe_fields);
        }

        Ok(())
    }

    pub fn build_runtime_filter_source(
        &mut self,
        runtime_filter_source: &RuntimeFilterSource,
    ) -> Result<()> {
        let state = self.build_runtime_filter_state(self.ctx.clone(), runtime_filter_source)?;
        self.expand_runtime_filter_source(&runtime_filter_source.right_side, state.clone())?;
        self.build_runtime_filter(&runtime_filter_source.left_side, state)?;
        Ok(())
    }

    fn expand_runtime_filter_source(
        &mut self,
        _right_side: &PhysicalPlan,
        state: Arc<RuntimeFilterState>,
    ) -> Result<()> {
        let pipeline = &mut self.pipelines[self.index.unwrap()];
        let output_size = pipeline.output_len();
        debug_assert!(output_size % 2 == 0);

        let mut items = Vec::with_capacity(output_size);
        //           Join
        //          /   \
        //        /      \
        //   RFSource     \
        //      /    \     \
        //     /      \     \
        // scan t1     scan t2
        for _ in 0..output_size / 2 {
            let input = InputPort::create();
            items.push(PipeItem::create(
                ProcessorPtr::create(TransformHashJoinBuild::try_create(
                    input.clone(),
                    self.join_state.as_ref().unwrap().clone(),
                    None,
                )?),
                vec![input],
                vec![],
            ));
            let input = InputPort::create();
            items.push(PipeItem::create(
                ProcessorPtr::create(Sinker::<SinkRuntimeFilterSource>::create(
                    input.clone(),
                    SinkRuntimeFilterSource::new(state.clone()),
                )),
                vec![input],
                vec![],
            ));
        }
        pipeline.add_pipe(Pipe::create(output_size, 0, items));
        Ok(())
    }

    fn build_runtime_filter(
        &mut self,
        left_side: &PhysicalPlan,
        state: Arc<RuntimeFilterState>,
    ) -> Result<()> {
        self.build_pipeline(left_side)?;
        self.main_pipeline.add_transform(|input, output| {
            let processor = TransformRuntimeFilter::create(input, output, state.clone());
            Ok(ProcessorPtr::create(processor))
        })?;
        Ok(())
    }

    fn build_runtime_filter_state(
        &self,
        ctx: Arc<QueryContext>,
        runtime_filter_source: &RuntimeFilterSource,
    ) -> Result<Arc<RuntimeFilterState>> {
        Ok(Arc::new(RuntimeFilterState::new(
            ctx,
            runtime_filter_source.left_runtime_filters.clone(),
            runtime_filter_source.right_runtime_filters.clone(),
        )))
    }

    pub(crate) fn build_materialized_cte(
        &mut self,
        materialized_cte: &MaterializedCte,
    ) -> Result<()> {
        self.expand_left_side_pipeline(
            &materialized_cte.left,
            materialized_cte.cte_idx,
            &materialized_cte.left_output_columns,
        )?;
        self.build_pipeline(&materialized_cte.right)
    }

    fn expand_left_side_pipeline(
        &mut self,
        left_side: &PhysicalPlan,
        cte_idx: IndexType,
        left_output_columns: &[ColumnBinding],
    ) -> Result<()> {
        let left_side_ctx = QueryContext::create_from(self.ctx.clone());
        let state = Arc::new(MaterializedCteState::new(self.ctx.clone()));
        self.cte_state.insert(cte_idx, state.clone());
        let mut left_side_builder = PipelineBuilder::create(
            self.func_ctx.clone(),
            self.settings.clone(),
            left_side_ctx,
            self.enable_profiling,
            self.proc_profs.clone(),
            self.main_pipeline.plans_scope.clone(),
        );
        left_side_builder.cte_state = self.cte_state.clone();
        let mut left_side_pipeline = left_side_builder.finalize(left_side)?;
        assert!(left_side_pipeline.main_pipeline.is_pulling_pipeline()?);

        PipelineBuilder::build_result_projection(
            &self.func_ctx,
            left_side.output_schema()?,
            left_output_columns,
            &mut left_side_pipeline.main_pipeline,
            false,
        )?;

        left_side_pipeline.main_pipeline.add_sink(|input| {
            let transform = Sinker::<MaterializedCteSink>::create(
                input,
                MaterializedCteSink::create(self.ctx.clone(), cte_idx, state.clone())?,
            );
            Ok(ProcessorPtr::create(transform))
        })?;
        self.pipelines
            .push(left_side_pipeline.main_pipeline.finalize());
        self.pipelines.extend(left_side_pipeline.sources_pipelines);
        Ok(())
    }
}
