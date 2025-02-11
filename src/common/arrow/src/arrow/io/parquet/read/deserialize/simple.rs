// Copyright 2020-2022 Jorge C. Leitão
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

use ethnum::I256;
use parquet2::schema::types::PhysicalType;
use parquet2::schema::types::PrimitiveLogicalType;
use parquet2::schema::types::PrimitiveType;
use parquet2::schema::types::TimeUnit as ParquetTimeUnit;
use parquet2::types::int96_to_i64_ns;

use super::super::ArrayIter;
use super::super::Pages;
use super::binary;
use super::boolean;
use super::fixed_size_binary;
use super::null;
use super::primitive;
use crate::arrow::array::Array;
use crate::arrow::array::DictionaryKey;
use crate::arrow::array::MutablePrimitiveArray;
use crate::arrow::array::PrimitiveArray;
use crate::arrow::datatypes::DataType;
use crate::arrow::datatypes::IntervalUnit;
use crate::arrow::datatypes::TimeUnit;
use crate::arrow::error::Error;
use crate::arrow::error::Result;
use crate::arrow::types::days_ms;
use crate::arrow::types::i256;
use crate::arrow::types::NativeType;

/// Converts an iterator of arrays to a trait object returning trait objects
#[inline]
fn dyn_iter<'a, A, I>(iter: I) -> ArrayIter<'a>
where
    A: Array,
    I: Iterator<Item = Result<A>> + Send + Sync + 'a,
{
    Box::new(iter.map(|x| x.map(|x| Box::new(x) as Box<dyn Array>)))
}

/// Converts an iterator of [MutablePrimitiveArray] into an iterator of [PrimitiveArray]
#[inline]
fn iden<T, I>(iter: I) -> impl Iterator<Item = Result<PrimitiveArray<T>>>
where
    T: NativeType,
    I: Iterator<Item = Result<MutablePrimitiveArray<T>>>,
{
    iter.map(|x| x.map(|x| x.into()))
}

#[inline]
fn op<T, I, F>(iter: I, op: F) -> impl Iterator<Item = Result<PrimitiveArray<T>>>
where
    T: NativeType,
    I: Iterator<Item = Result<MutablePrimitiveArray<T>>>,
    F: Fn(T) -> T + Copy,
{
    iter.map(move |x| {
        x.map(move |mut x| {
            x.values_mut_slice().iter_mut().for_each(|x| *x = op(*x));
            x.into()
        })
    })
}

/// An iterator adapter that maps an iterator of Pages into an iterator of Arrays
/// of [`DataType`] `data_type` and length `chunk_size`.
pub fn page_iter_to_arrays<'a, I: Pages + 'a>(
    pages: I,
    type_: &PrimitiveType,
    data_type: DataType,
    chunk_size: Option<usize>,
    num_rows: usize,
) -> Result<ArrayIter<'a>> {
    use DataType::*;

    let physical_type = &type_.physical_type;
    let logical_type = &type_.logical_type;

    Ok(match (physical_type, data_type.to_logical_type()) {
        (_, Null) => null::iter_to_arrays(pages, data_type, chunk_size, num_rows),
        (PhysicalType::Boolean, Boolean) => {
            dyn_iter(boolean::Iter::new(pages, data_type, chunk_size, num_rows))
        }
        (PhysicalType::Int32, UInt8) => dyn_iter(iden(primitive::IntegerIter::new(
            pages,
            data_type,
            num_rows,
            chunk_size,
            |x: i32| x as u8,
        ))),
        (PhysicalType::Int32, UInt16) => dyn_iter(iden(primitive::IntegerIter::new(
            pages,
            data_type,
            num_rows,
            chunk_size,
            |x: i32| x as u16,
        ))),
        (PhysicalType::Int32, UInt32) => dyn_iter(iden(primitive::IntegerIter::new(
            pages,
            data_type,
            num_rows,
            chunk_size,
            |x: i32| x as u32,
        ))),
        (PhysicalType::Int64, UInt32) => dyn_iter(iden(primitive::IntegerIter::new(
            pages,
            data_type,
            num_rows,
            chunk_size,
            |x: i64| x as u32,
        ))),
        (PhysicalType::Int32, Int8) => dyn_iter(iden(primitive::IntegerIter::new(
            pages,
            data_type,
            num_rows,
            chunk_size,
            |x: i32| x as i8,
        ))),
        (PhysicalType::Int32, Int16) => dyn_iter(iden(primitive::IntegerIter::new(
            pages,
            data_type,
            num_rows,
            chunk_size,
            |x: i32| x as i16,
        ))),
        (PhysicalType::Int32, Int32 | Date32 | Time32(_)) => dyn_iter(iden(
            primitive::IntegerIter::new(pages, data_type, num_rows, chunk_size, |x: i32| x),
        )),
        (PhysicalType::Int64 | PhysicalType::Int96, Timestamp(time_unit, _)) => {
            let time_unit = *time_unit;
            return timestamp(
                pages,
                physical_type,
                logical_type,
                data_type,
                num_rows,
                chunk_size,
                time_unit,
            );
        }
        (PhysicalType::FixedLenByteArray(_), FixedSizeBinary(_)) => dyn_iter(
            fixed_size_binary::Iter::new(pages, data_type, num_rows, chunk_size),
        ),
        (PhysicalType::FixedLenByteArray(12), Interval(IntervalUnit::YearMonth)) => {
            let n = 12;
            let pages = fixed_size_binary::Iter::new(
                pages,
                DataType::FixedSizeBinary(n),
                num_rows,
                chunk_size,
            );

            let pages = pages.map(move |maybe_array| {
                let array = maybe_array?;
                let values = array
                    .values()
                    .chunks_exact(n)
                    .map(|value: &[u8]| i32::from_le_bytes(value[..4].try_into().unwrap()))
                    .collect::<Vec<_>>();
                let validity = array.validity().cloned();

                PrimitiveArray::<i32>::try_new(data_type.clone(), values.into(), validity)
            });

            let arrays = pages.map(|x| x.map(|x| x.boxed()));

            Box::new(arrays) as _
        }
        (PhysicalType::FixedLenByteArray(12), Interval(IntervalUnit::DayTime)) => {
            let n = 12;
            let pages = fixed_size_binary::Iter::new(
                pages,
                DataType::FixedSizeBinary(n),
                num_rows,
                chunk_size,
            );

            let pages = pages.map(move |maybe_array| {
                let array = maybe_array?;
                let values = array
                    .values()
                    .chunks_exact(n)
                    .map(super::super::convert_days_ms)
                    .collect::<Vec<_>>();
                let validity = array.validity().cloned();

                PrimitiveArray::<days_ms>::try_new(data_type.clone(), values.into(), validity)
            });

            let arrays = pages.map(|x| x.map(|x| x.boxed()));

            Box::new(arrays) as _
        }
        (PhysicalType::Int32, Decimal(_, _)) => dyn_iter(iden(primitive::IntegerIter::new(
            pages,
            data_type,
            num_rows,
            chunk_size,
            |x: i32| x as i128,
        ))),
        (PhysicalType::Int64, Decimal(_, _)) => dyn_iter(iden(primitive::IntegerIter::new(
            pages,
            data_type,
            num_rows,
            chunk_size,
            |x: i64| x as i128,
        ))),
        (PhysicalType::FixedLenByteArray(n), Decimal(_, _)) if *n > 16 => {
            return Err(Error::NotYetImplemented(format!(
                "Can't decode Decimal128 type from Fixed Size Byte Array of len {n:?}"
            )));
        }
        (PhysicalType::FixedLenByteArray(n), Decimal(_, _)) => {
            let n = *n;

            let pages = fixed_size_binary::Iter::new(
                pages,
                DataType::FixedSizeBinary(n),
                num_rows,
                chunk_size,
            );

            let pages = pages.map(move |maybe_array| {
                let array = maybe_array?;
                let values = array
                    .values()
                    .chunks_exact(n)
                    .map(|value: &[u8]| super::super::convert_i128(value, n))
                    .collect::<Vec<_>>();
                let validity = array.validity().cloned();

                PrimitiveArray::<i128>::try_new(data_type.clone(), values.into(), validity)
            });

            let arrays = pages.map(|x| x.map(|x| x.boxed()));

            Box::new(arrays) as _
        }
        (PhysicalType::Int32, Decimal256(_, _)) => dyn_iter(iden(primitive::IntegerIter::new(
            pages,
            data_type,
            num_rows,
            chunk_size,
            |x: i32| i256(I256::new(x as i128)),
        ))),
        (PhysicalType::Int64, Decimal256(_, _)) => dyn_iter(iden(primitive::IntegerIter::new(
            pages,
            data_type,
            num_rows,
            chunk_size,
            |x: i64| i256(I256::new(x as i128)),
        ))),
        (PhysicalType::FixedLenByteArray(n), Decimal256(_, _)) if *n <= 16 => {
            let n = *n;

            let pages = fixed_size_binary::Iter::new(
                pages,
                DataType::FixedSizeBinary(n),
                num_rows,
                chunk_size,
            );

            let pages = pages.map(move |maybe_array| {
                let array = maybe_array?;
                let values = array
                    .values()
                    .chunks_exact(n)
                    .map(|value: &[u8]| i256(I256::new(super::super::convert_i128(value, n))))
                    .collect::<Vec<_>>();
                let validity = array.validity().cloned();

                PrimitiveArray::<i256>::try_new(data_type.clone(), values.into(), validity)
            });

            let arrays = pages.map(|x| x.map(|x| x.boxed()));

            Box::new(arrays) as _
        }
        (PhysicalType::FixedLenByteArray(n), Decimal256(_, _)) if *n <= 32 => {
            let n = *n;

            let pages = fixed_size_binary::Iter::new(
                pages,
                DataType::FixedSizeBinary(n),
                num_rows,
                chunk_size,
            );

            let pages = pages.map(move |maybe_array| {
                let array = maybe_array?;
                let values = array
                    .values()
                    .chunks_exact(n)
                    .map(super::super::convert_i256)
                    .collect::<Vec<_>>();
                let validity = array.validity().cloned();

                PrimitiveArray::<i256>::try_new(data_type.clone(), values.into(), validity)
            });

            let arrays = pages.map(|x| x.map(|x| x.boxed()));

            Box::new(arrays) as _
        }
        (PhysicalType::FixedLenByteArray(n), Decimal256(_, _)) if *n > 32 => {
            return Err(Error::NotYetImplemented(format!(
                "Can't decode Decimal256 type from Fixed Size Byte Array of len {n:?}"
            )));
        }
        (PhysicalType::Int32, Date64) => dyn_iter(iden(primitive::IntegerIter::new(
            pages,
            data_type,
            num_rows,
            chunk_size,
            |x: i32| x as i64 * 86400000,
        ))),
        (PhysicalType::Int64, Date64) => dyn_iter(iden(primitive::IntegerIter::new(
            pages,
            data_type,
            num_rows,
            chunk_size,
            |x: i64| x,
        ))),
        (PhysicalType::Int64, Int64 | Time64(_) | Duration(_)) => dyn_iter(iden(
            primitive::IntegerIter::new(pages, data_type, num_rows, chunk_size, |x: i64| x),
        )),
        (PhysicalType::Int64, UInt64) => dyn_iter(iden(primitive::IntegerIter::new(
            pages,
            data_type,
            num_rows,
            chunk_size,
            |x: i64| x as u64,
        ))),
        (PhysicalType::Float, Float32) => dyn_iter(iden(primitive::Iter::new(
            pages,
            data_type,
            num_rows,
            chunk_size,
            |x: f32| x,
        ))),
        (PhysicalType::Double, Float64) => dyn_iter(iden(primitive::Iter::new(
            pages,
            data_type,
            num_rows,
            chunk_size,
            |x: f64| x,
        ))),

        (PhysicalType::ByteArray, Utf8 | Binary) => Box::new(binary::Iter::<i32, _>::new(
            pages, data_type, chunk_size, num_rows,
        )),
        (PhysicalType::ByteArray, LargeBinary | LargeUtf8) => Box::new(
            binary::Iter::<i64, _>::new(pages, data_type, chunk_size, num_rows),
        ),

        (_, Dictionary(key_type, _, _)) => {
            return match_integer_type!(key_type, |$K| {
                dict_read::<$K, _>(pages, physical_type, logical_type, data_type, num_rows, chunk_size)
            });
        }
        (from, to) => {
            return Err(Error::NotYetImplemented(format!(
                "Reading parquet type {from:?} to {to:?} still not implemented"
            )));
        }
    })
}

/// Unify the timestamp unit from parquet TimeUnit into arrow's TimeUnit
/// Returns (a int64 factor, is_multiplier)
fn unify_timestamp_unit(
    logical_type: &Option<PrimitiveLogicalType>,
    time_unit: TimeUnit,
) -> (i64, bool) {
    if let Some(PrimitiveLogicalType::Timestamp { unit, .. }) = logical_type {
        match (*unit, time_unit) {
            (ParquetTimeUnit::Milliseconds, TimeUnit::Millisecond)
            | (ParquetTimeUnit::Microseconds, TimeUnit::Microsecond)
            | (ParquetTimeUnit::Nanoseconds, TimeUnit::Nanosecond) => (1, true),

            (ParquetTimeUnit::Milliseconds, TimeUnit::Second)
            | (ParquetTimeUnit::Microseconds, TimeUnit::Millisecond)
            | (ParquetTimeUnit::Nanoseconds, TimeUnit::Microsecond) => (1000, false),

            (ParquetTimeUnit::Microseconds, TimeUnit::Second)
            | (ParquetTimeUnit::Nanoseconds, TimeUnit::Millisecond) => (1_000_000, false),

            (ParquetTimeUnit::Nanoseconds, TimeUnit::Second) => (1_000_000_000, false),

            (ParquetTimeUnit::Milliseconds, TimeUnit::Microsecond)
            | (ParquetTimeUnit::Microseconds, TimeUnit::Nanosecond) => (1_000, true),

            (ParquetTimeUnit::Milliseconds, TimeUnit::Nanosecond) => (1_000_000, true),
        }
    } else {
        (1, true)
    }
}

#[inline]
pub fn int96_to_i64_us(value: [u32; 3]) -> i64 {
    const JULIAN_DAY_OF_EPOCH: i64 = 2_440_588;
    const SECONDS_PER_DAY: i64 = 86_400;
    const MICROS_PER_SECOND: i64 = 1_000_000;

    let day = value[2] as i64;
    let microseconds = (((value[1] as i64) << 32) + value[0] as i64) / 1_000;
    let seconds = (day - JULIAN_DAY_OF_EPOCH) * SECONDS_PER_DAY;

    seconds * MICROS_PER_SECOND + microseconds
}

#[inline]
pub fn int96_to_i64_ms(value: [u32; 3]) -> i64 {
    const JULIAN_DAY_OF_EPOCH: i64 = 2_440_588;
    const SECONDS_PER_DAY: i64 = 86_400;
    const MILLIS_PER_SECOND: i64 = 1_000;

    let day = value[2] as i64;
    let milliseconds = (((value[1] as i64) << 32) + value[0] as i64) / 1_000_000;
    let seconds = (day - JULIAN_DAY_OF_EPOCH) * SECONDS_PER_DAY;

    seconds * MILLIS_PER_SECOND + milliseconds
}

#[inline]
pub fn int96_to_i64_s(value: [u32; 3]) -> i64 {
    const JULIAN_DAY_OF_EPOCH: i64 = 2_440_588;
    const SECONDS_PER_DAY: i64 = 86_400;

    let day = value[2] as i64;
    let seconds = (((value[1] as i64) << 32) + value[0] as i64) / 1_000_000_000;
    let day_seconds = (day - JULIAN_DAY_OF_EPOCH) * SECONDS_PER_DAY;

    day_seconds + seconds
}

fn timestamp<'a, I: Pages + 'a>(
    pages: I,
    physical_type: &PhysicalType,
    logical_type: &Option<PrimitiveLogicalType>,
    data_type: DataType,
    num_rows: usize,
    chunk_size: Option<usize>,
    time_unit: TimeUnit,
) -> Result<ArrayIter<'a>> {
    if physical_type == &PhysicalType::Int96 {
        return match time_unit {
            TimeUnit::Nanosecond => Ok(dyn_iter(iden(primitive::Iter::new(
                pages,
                data_type,
                num_rows,
                chunk_size,
                int96_to_i64_ns,
            )))),
            TimeUnit::Microsecond => Ok(dyn_iter(iden(primitive::Iter::new(
                pages,
                data_type,
                num_rows,
                chunk_size,
                int96_to_i64_us,
            )))),
            TimeUnit::Millisecond => Ok(dyn_iter(iden(primitive::Iter::new(
                pages,
                data_type,
                num_rows,
                chunk_size,
                int96_to_i64_ms,
            )))),
            TimeUnit::Second => Ok(dyn_iter(iden(primitive::Iter::new(
                pages,
                data_type,
                num_rows,
                chunk_size,
                int96_to_i64_s,
            )))),
        };
    };

    if physical_type != &PhysicalType::Int64 {
        return Err(Error::nyi(
            "Can't decode a timestamp from a non-int64 parquet type",
        ));
    }

    let iter = primitive::IntegerIter::new(pages, data_type, num_rows, chunk_size, |x: i64| x);
    let (factor, is_multiplier) = unify_timestamp_unit(logical_type, time_unit);
    match (factor, is_multiplier) {
        (1, _) => Ok(dyn_iter(iden(iter))),
        (a, true) => Ok(dyn_iter(op(iter, move |x| x * a))),
        (a, false) => Ok(dyn_iter(op(iter, move |x| x / a))),
    }
}

fn timestamp_dict<'a, K: DictionaryKey, I: Pages + 'a>(
    pages: I,
    physical_type: &PhysicalType,
    logical_type: &Option<PrimitiveLogicalType>,
    data_type: DataType,
    num_rows: usize,
    chunk_size: Option<usize>,
    time_unit: TimeUnit,
) -> Result<ArrayIter<'a>> {
    if physical_type == &PhysicalType::Int96 {
        let logical_type = PrimitiveLogicalType::Timestamp {
            unit: ParquetTimeUnit::Nanoseconds,
            is_adjusted_to_utc: false,
        };
        let (factor, is_multiplier) = unify_timestamp_unit(&Some(logical_type), time_unit);
        return match (factor, is_multiplier) {
            (a, true) => Ok(dyn_iter(primitive::DictIter::<K, _, _, _, _>::new(
                pages,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                num_rows,
                chunk_size,
                move |x| int96_to_i64_ns(x) * a,
            ))),
            (a, false) => Ok(dyn_iter(primitive::DictIter::<K, _, _, _, _>::new(
                pages,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                num_rows,
                chunk_size,
                move |x| int96_to_i64_ns(x) / a,
            ))),
        };
    };

    let (factor, is_multiplier) = unify_timestamp_unit(logical_type, time_unit);
    match (factor, is_multiplier) {
        (a, true) => Ok(dyn_iter(primitive::DictIter::<K, _, _, _, _>::new(
            pages,
            data_type,
            num_rows,
            chunk_size,
            move |x: i64| x * a,
        ))),
        (a, false) => Ok(dyn_iter(primitive::DictIter::<K, _, _, _, _>::new(
            pages,
            data_type,
            num_rows,
            chunk_size,
            move |x: i64| x / a,
        ))),
    }
}

fn dict_read<'a, K: DictionaryKey, I: Pages + 'a>(
    iter: I,
    physical_type: &PhysicalType,
    logical_type: &Option<PrimitiveLogicalType>,
    data_type: DataType,
    num_rows: usize,
    chunk_size: Option<usize>,
) -> Result<ArrayIter<'a>> {
    use DataType::*;
    let values_data_type = if let Dictionary(_, v, _) = &data_type {
        v.as_ref()
    } else {
        panic!()
    };

    Ok(match (physical_type, values_data_type.to_logical_type()) {
        (PhysicalType::Int32, UInt8) => dyn_iter(primitive::DictIter::<K, _, _, _, _>::new(
            iter,
            data_type,
            num_rows,
            chunk_size,
            |x: i32| x as u8,
        )),
        (PhysicalType::Int32, UInt16) => dyn_iter(primitive::DictIter::<K, _, _, _, _>::new(
            iter,
            data_type,
            num_rows,
            chunk_size,
            |x: i32| x as u16,
        )),
        (PhysicalType::Int32, UInt32) => dyn_iter(primitive::DictIter::<K, _, _, _, _>::new(
            iter,
            data_type,
            num_rows,
            chunk_size,
            |x: i32| x as u32,
        )),
        (PhysicalType::Int64, UInt64) => dyn_iter(primitive::DictIter::<K, _, _, _, _>::new(
            iter,
            data_type,
            num_rows,
            chunk_size,
            |x: i64| x as u64,
        )),
        (PhysicalType::Int32, Int8) => dyn_iter(primitive::DictIter::<K, _, _, _, _>::new(
            iter,
            data_type,
            num_rows,
            chunk_size,
            |x: i32| x as i8,
        )),
        (PhysicalType::Int32, Int16) => dyn_iter(primitive::DictIter::<K, _, _, _, _>::new(
            iter,
            data_type,
            num_rows,
            chunk_size,
            |x: i32| x as i16,
        )),
        (PhysicalType::Int32, Int32 | Date32 | Time32(_) | Interval(IntervalUnit::YearMonth)) => {
            dyn_iter(primitive::DictIter::<K, _, _, _, _>::new(
                iter,
                data_type,
                num_rows,
                chunk_size,
                |x: i32| x,
            ))
        }

        (PhysicalType::Int64, Timestamp(time_unit, _)) => {
            let time_unit = *time_unit;
            return timestamp_dict::<K, _>(
                iter,
                physical_type,
                logical_type,
                data_type,
                num_rows,
                chunk_size,
                time_unit,
            );
        }

        (PhysicalType::Int64, Int64 | Date64 | Time64(_) | Duration(_)) => {
            dyn_iter(primitive::DictIter::<K, _, _, _, _>::new(
                iter,
                data_type,
                num_rows,
                chunk_size,
                |x: i64| x,
            ))
        }
        (PhysicalType::Float, Float32) => dyn_iter(primitive::DictIter::<K, _, _, _, _>::new(
            iter,
            data_type,
            num_rows,
            chunk_size,
            |x: f32| x,
        )),
        (PhysicalType::Double, Float64) => dyn_iter(primitive::DictIter::<K, _, _, _, _>::new(
            iter,
            data_type,
            num_rows,
            chunk_size,
            |x: f64| x,
        )),

        (PhysicalType::ByteArray, Utf8 | Binary) => dyn_iter(binary::DictIter::<K, i32, _>::new(
            iter, data_type, num_rows, chunk_size,
        )),
        (PhysicalType::ByteArray, LargeUtf8 | LargeBinary) => dyn_iter(
            binary::DictIter::<K, i64, _>::new(iter, data_type, num_rows, chunk_size),
        ),
        (PhysicalType::FixedLenByteArray(_), FixedSizeBinary(_)) => dyn_iter(
            fixed_size_binary::DictIter::<K, _>::new(iter, data_type, num_rows, chunk_size),
        ),
        other => {
            return Err(Error::nyi(format!(
                "Reading dictionaries of type {other:?}"
            )));
        }
    })
}
