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

use std::collections::BTreeMap;
use std::fmt::Display;
use std::fmt::Formatter;

use common_base::base::mask_string;

use crate::ast::Identifier;

#[derive(Debug, Clone, PartialEq)]
pub struct CreateConnectionStmt {
    pub if_not_exists: bool,
    pub name: Identifier,
    pub storage_type: String,
    pub storage_params: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropConnectionStmt {
    pub if_exists: bool,
    pub name: Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescribeConnectionStmt {
    pub name: Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShowConnectionsStmt {}

impl Display for CreateConnectionStmt {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "CREATE CONNECTION ")?;
        if self.if_not_exists {
            write!(f, "IF NOT EXISTS ")?;
        }
        write!(f, "{} ", self.name)?;
        write!(f, "STORAGE_TYPE = {} ", self.storage_type)?;
        for (k, v) in &self.storage_params {
            write!(f, "{} = {}", k, mask_string(v, 3))?;
        }
        Ok(())
    }
}

impl Display for DropConnectionStmt {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "CREATE CONNECTION ")?;
        if self.if_exists {
            write!(f, "IF NOT EXISTS ")?;
        }
        write!(f, "{} ", self.name)
    }
}

impl Display for DescribeConnectionStmt {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "CREATE CONNECTION {} ", self.name)
    }
}

impl Display for ShowConnectionsStmt {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "SHOW CONNECTIONS")
    }
}
