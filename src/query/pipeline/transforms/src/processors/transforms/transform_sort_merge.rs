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

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::intrinsics::unlikely;
use std::marker::PhantomData;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use common_exception::ErrorCode;
use common_exception::Result;
use common_expression::row::RowConverter as CommonRowConverter;
use common_expression::types::string::StringColumn;
use common_expression::types::DataType;
use common_expression::types::DateType;
use common_expression::types::NumberDataType;
use common_expression::types::NumberType;
use common_expression::types::StringType;
use common_expression::types::TimestampType;
use common_expression::with_number_mapped_type;
use common_expression::BlockEntry;
use common_expression::DataBlock;
use common_expression::DataSchemaRef;
use common_expression::SortColumnDescription;
use common_expression::Value;
use common_pipeline_core::processors::InputPort;
use common_pipeline_core::processors::OutputPort;
use common_pipeline_core::processors::Processor;

use super::sort::Cursor;
use super::sort::RowConverter;
use super::sort::Rows;
use super::sort::SimpleRowConverter;
use super::sort::SimpleRows;
use super::Compactor;
use super::TransformCompact;

/// Merge sort blocks without limit.
///
/// For merge sort with limit, see [`super::transform_sort_merge_limit`]
pub struct SortMergeCompactor<R, Converter> {
    block_size: usize,
    row_converter: Converter,
    order_by_cols: Vec<usize>,

    aborting: Arc<AtomicBool>,

    /// If the next transform of current transform is [`super::transform_multi_sort_merge::MultiSortMergeProcessor`],
    /// we can generate the order column to avoid the extra converting in the next transform.
    gen_order_col: bool,

    _c: PhantomData<Converter>,
    _r: PhantomData<R>,
}

impl<R, Converter> SortMergeCompactor<R, Converter>
where
    R: Rows,
    Converter: RowConverter<R>,
{
    pub fn try_create(
        schema: DataSchemaRef,
        block_size: usize,
        sort_desc: Vec<SortColumnDescription>,
        gen_order_col: bool,
    ) -> Result<Self> {
        let order_by_cols = sort_desc.iter().map(|i| i.offset).collect::<Vec<_>>();
        let row_converter = Converter::create(sort_desc, schema)?;
        Ok(SortMergeCompactor {
            order_by_cols,
            row_converter,
            block_size,
            aborting: Arc::new(AtomicBool::new(false)),
            gen_order_col,
            _c: PhantomData,
            _r: PhantomData,
        })
    }
}

impl<R, Converter> Compactor for SortMergeCompactor<R, Converter>
where
    R: Rows,
    Converter: RowConverter<R>,
{
    fn name() -> &'static str {
        "SortMergeTransform"
    }

    fn interrupt(&self) {
        self.aborting.store(true, Ordering::Release);
    }

    fn compact_final(&mut self, blocks: Vec<DataBlock>) -> Result<Vec<DataBlock>> {
        if blocks.is_empty() {
            return Ok(vec![]);
        }

        let output_size = blocks.iter().map(|b| b.num_rows()).sum::<usize>();
        if output_size == 0 {
            return Ok(vec![]);
        }

        let mut blocks = blocks
            .into_iter()
            .filter(|b| !b.is_empty())
            .collect::<Vec<_>>();

        if blocks.len() == 1 {
            if self.gen_order_col {
                let block = blocks.get_mut(0).ok_or(ErrorCode::Internal("It's a bug"))?;
                let columns = self
                    .order_by_cols
                    .iter()
                    .map(|i| block.get_by_offset(*i).clone())
                    .collect::<Vec<_>>();
                let rows = self.row_converter.convert(&columns, block.num_rows())?;
                let order_col = rows.to_column();
                block.add_column(BlockEntry {
                    data_type: order_col.data_type(),
                    value: Value::Column(order_col),
                });
            }
            return Ok(blocks);
        }

        let output_block_num = output_size.div_ceil(self.block_size);
        let mut output_blocks = Vec::with_capacity(output_block_num);
        let mut output_indices = Vec::with_capacity(output_size);
        let mut heap: BinaryHeap<Reverse<Cursor<R>>> = BinaryHeap::with_capacity(blocks.len());

        // 1. Put all blocks into a min-heap.
        for (i, block) in blocks.iter_mut().enumerate() {
            let columns = self
                .order_by_cols
                .iter()
                .map(|i| block.get_by_offset(*i).clone())
                .collect::<Vec<_>>();
            let rows = self.row_converter.convert(&columns, block.num_rows())?;

            if self.gen_order_col {
                let order_col = rows.to_column();
                block.add_column(BlockEntry {
                    data_type: order_col.data_type(),
                    value: Value::Column(order_col),
                });
            }
            let cursor = Cursor::new(i, rows);
            heap.push(Reverse(cursor));
        }

        // 2. Drain the heap
        while let Some(Reverse(mut cursor)) = heap.pop() {
            if unlikely(self.aborting.load(Ordering::Relaxed)) {
                return Err(ErrorCode::AbortedQuery(
                    "Aborted query, because the server is shutting down or the query was killed.",
                ));
            }

            let block_idx = cursor.input_index;
            if heap.is_empty() {
                // If there is no other block in the heap, we can drain the whole block.
                while !cursor.is_finished() {
                    output_indices.push((block_idx, cursor.advance()));
                }
            } else {
                let next_cursor = &heap.peek().unwrap().0;
                // If the last row of current block is smaller than the next cursor,
                // we can drain the whole block.
                if cursor.last().le(&next_cursor.current()) {
                    while !cursor.is_finished() {
                        output_indices.push((block_idx, cursor.advance()));
                    }
                } else {
                    while !cursor.is_finished() && cursor.le(next_cursor) {
                        // If the cursor is smaller than the next cursor, don't need to push the cursor back to the heap.
                        output_indices.push((block_idx, cursor.advance()));
                    }
                    if !cursor.is_finished() {
                        heap.push(Reverse(cursor));
                    }
                }
            }
        }

        // 3. Build final blocks from `output_indices`.
        for i in 0..output_block_num {
            if unlikely(self.aborting.load(Ordering::Relaxed)) {
                return Err(ErrorCode::AbortedQuery(
                    "Aborted query, because the server is shutting down or the query was killed.",
                ));
            }

            let start = i * self.block_size;
            let end = (start + self.block_size).min(output_indices.len());
            // Convert indices to merge slice.
            let mut merge_slices = Vec::with_capacity(output_indices.len());
            let (block_idx, row_idx) = output_indices[start];
            merge_slices.push((block_idx, row_idx, 1));
            for (block_idx, row_idx) in output_indices.iter().take(end).skip(start + 1) {
                if *block_idx == merge_slices.last().unwrap().0 {
                    // If the block index is the same as the last one, we can merge them.
                    merge_slices.last_mut().unwrap().2 += 1;
                } else {
                    merge_slices.push((*block_idx, *row_idx, 1));
                }
            }
            let block = DataBlock::take_by_slices_limit_from_blocks(&blocks, &merge_slices, None);
            output_blocks.push(block);
        }

        Ok(output_blocks)
    }
}

type SimpleDateCompactor = SortMergeCompactor<SimpleRows<DateType>, SimpleRowConverter<DateType>>;
type SimpleDateSort = TransformCompact<SimpleDateCompactor>;

type SimpleTimestampCompactor =
    SortMergeCompactor<SimpleRows<TimestampType>, SimpleRowConverter<TimestampType>>;
type SimpleTimestampSort = TransformCompact<SimpleTimestampCompactor>;

type SimpleStringCompactor =
    SortMergeCompactor<SimpleRows<StringType>, SimpleRowConverter<StringType>>;
type SimpleStringSort = TransformCompact<SimpleStringCompactor>;

type CommonCompactor = SortMergeCompactor<StringColumn, CommonRowConverter>;
type CommonSort = TransformCompact<CommonCompactor>;

pub fn try_create_transform_sort_merge(
    input: Arc<InputPort>,
    output: Arc<OutputPort>,
    output_schema: DataSchemaRef,
    block_size: usize,
    sort_desc: Vec<SortColumnDescription>,
    gen_order_col: bool,
) -> Result<Box<dyn Processor>> {
    if sort_desc.len() == 1 {
        let sort_type = output_schema.field(sort_desc[0].offset).data_type();
        match sort_type {
            DataType::Number(num_ty) => with_number_mapped_type!(|NUM_TYPE| match num_ty {
                NumberDataType::NUM_TYPE => TransformCompact::<
                    SortMergeCompactor<
                        SimpleRows<NumberType<NUM_TYPE>>,
                        SimpleRowConverter<NumberType<NUM_TYPE>>,
                    >,
                >::try_create(
                    input,
                    output,
                    SortMergeCompactor::<
                        SimpleRows<NumberType<NUM_TYPE>>,
                        SimpleRowConverter<NumberType<NUM_TYPE>>,
                    >::try_create(
                        output_schema, block_size, sort_desc, gen_order_col
                    )?
                ),
            }),
            DataType::Date => SimpleDateSort::try_create(
                input,
                output,
                SimpleDateCompactor::try_create(
                    output_schema,
                    block_size,
                    sort_desc,
                    gen_order_col,
                )?,
            ),
            DataType::Timestamp => SimpleTimestampSort::try_create(
                input,
                output,
                SimpleTimestampCompactor::try_create(
                    output_schema,
                    block_size,
                    sort_desc,
                    gen_order_col,
                )?,
            ),
            DataType::String => SimpleStringSort::try_create(
                input,
                output,
                SimpleStringCompactor::try_create(
                    output_schema,
                    block_size,
                    sort_desc,
                    gen_order_col,
                )?,
            ),
            _ => CommonSort::try_create(
                input,
                output,
                CommonCompactor::try_create(output_schema, block_size, sort_desc, gen_order_col)?,
            ),
        }
    } else {
        CommonSort::try_create(
            input,
            output,
            CommonCompactor::try_create(output_schema, block_size, sort_desc, gen_order_col)?,
        )
    }
}

pub fn sort_merge(
    data_schema: DataSchemaRef,
    block_size: usize,
    sort_desc: Vec<SortColumnDescription>,
    data_blocks: Vec<DataBlock>,
) -> Result<Vec<DataBlock>> {
    let mut compactor = CommonCompactor::try_create(data_schema, block_size, sort_desc, false)?;
    compactor.compact_final(data_blocks)
}
