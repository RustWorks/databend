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

use common_arrow::arrow::array::BooleanArray;
use common_arrow::arrow::datatypes::DataType;
use common_arrow::arrow::datatypes::Field;
use common_arrow::arrow::scalar::ListScalar;
use common_arrow::arrow::scalar::Scalar;

#[allow(clippy::eq_op)]
#[test]
fn equal() {
    let dt = DataType::List(Box::new(Field::new("a", DataType::Boolean, true)));
    let a = ListScalar::<i32>::new(
        dt.clone(),
        Some(BooleanArray::from_slice([true, false]).boxed()),
    );
    let b = ListScalar::<i32>::new(dt.clone(), None);
    assert_eq!(a, a);
    assert_eq!(b, b);
    assert!(a != b);
    let b = ListScalar::<i32>::new(dt, Some(BooleanArray::from_slice([true, true]).boxed()));
    assert!(a != b);
    assert_eq!(b, b);
}

#[test]
fn basics() {
    let dt = DataType::List(Box::new(Field::new("a", DataType::Boolean, true)));
    let a = ListScalar::<i32>::new(
        dt.clone(),
        Some(BooleanArray::from_slice([true, false]).boxed()),
    );

    assert_eq!(BooleanArray::from_slice([true, false]), a.values().as_ref());
    assert_eq!(a.data_type(), &dt);
    assert!(a.is_valid());

    let _: &dyn std::any::Any = a.as_any();
}
