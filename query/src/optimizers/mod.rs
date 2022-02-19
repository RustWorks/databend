// Copyright 2021 Datafuse Labs.
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

mod metrics;
mod optimizer;
mod optimizer_constant_folding;
mod optimizer_expression_transform;
mod optimizer_scatters;
mod optimizer_statistics_exact;
mod optimizer_top_n_push_down;

pub use optimizer::Optimizer;
pub use optimizer::Optimizers;
pub use optimizer_constant_folding::ConstantFoldingOptimizer;
pub use optimizer_expression_transform::ExprTransformOptimizer;
pub use optimizer_scatters::ScattersOptimizer;
pub use optimizer_statistics_exact::StatisticsExactOptimizer;
pub use optimizer_top_n_push_down::TopNPushDownOptimizer;
