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

mod iterator;
mod mutable;

use common_arrow::arrow::array::*;
use common_arrow::arrow::bitmap::Bitmap;
use common_arrow::arrow::datatypes::*;

#[test]
fn debug() {
    let boolean = BooleanArray::from_slice([false, false, true, true]).boxed();
    let int = Int32Array::from_slice([42, 28, 19, 31]).boxed();

    let fields = vec![
        Field::new("b", DataType::Boolean, false),
        Field::new("c", DataType::Int32, false),
    ];

    let array = StructArray::new(
        DataType::Struct(fields),
        vec![boolean.clone(), int.clone()],
        Some(Bitmap::from([true, true, false, true])),
    );
    assert_eq!(
        format!("{array:?}"),
        "StructArray[{b: false, c: 42}, {b: false, c: 28}, None, {b: true, c: 31}]"
    );
}
