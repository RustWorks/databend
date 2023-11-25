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

use std::collections::HashSet;

use common_meta_app::principal::GrantObject;
use common_meta_app::principal::RoleInfo;
use common_meta_app::principal::UserGrantSet;
use common_meta_app::principal::UserInfo;

/// GrantObjectVisibilityChecker is used to check whether a user has the privilege to access a
/// database or table.
/// It is used in `SHOW DATABASES` and `SHOW TABLES` statements.
pub struct GrantObjectVisibilityChecker {
    granted_global: bool,
    granted_databases: HashSet<(String, String)>,
    granted_tables: HashSet<(String, String, String)>,
    extra_databases: HashSet<(String, String)>,
    granted_udfs: HashSet<String>,
    granted_stages: HashSet<String>,
}

impl GrantObjectVisibilityChecker {
    pub fn new(user: &UserInfo, available_roles: &Vec<RoleInfo>) -> Self {
        let mut granted_global = false;
        let mut granted_databases = HashSet::new();
        let mut granted_tables = HashSet::new();
        let mut granted_udfs = HashSet::new();
        let mut granted_stages = HashSet::new();
        let mut extra_databases = HashSet::new();

        let mut grant_sets: Vec<&UserGrantSet> = vec![&user.grants];
        for role in available_roles {
            grant_sets.push(&role.grants);
        }

        for grant_set in grant_sets {
            for ent in grant_set.entries() {
                match ent.object() {
                    GrantObject::Global => {
                        granted_global = true;
                    }
                    GrantObject::Database(catalog, db) => {
                        granted_databases.insert((catalog.to_string(), db.to_string()));
                    }
                    GrantObject::Table(catalog, db, table) => {
                        granted_tables.insert((
                            catalog.to_string(),
                            db.to_string(),
                            table.to_string(),
                        ));
                        // if table is visible, the table's database is also treated as visible
                        extra_databases.insert((catalog.to_string(), db.to_string()));
                    }
                    GrantObject::UDF(udf) => {
                        granted_udfs.insert(udf.to_string());
                    }
                    GrantObject::Stage(stage) => {
                        granted_stages.insert(stage.to_string());
                    }
                }
            }
        }

        Self {
            granted_global,
            granted_databases,
            granted_tables,
            extra_databases,
            granted_udfs,
            granted_stages,
        }
    }

    pub fn check_stage_visibility(&self, stage: &str) -> bool {
        if self.granted_global {
            return true;
        }

        if self.granted_stages.contains(stage) {
            return true;
        }
        false
    }

    pub fn check_udf_visibility(&self, udf: &str) -> bool {
        if self.granted_global {
            return true;
        }

        if self.granted_udfs.contains(udf) {
            return true;
        }
        false
    }

    pub fn check_database_visibility(&self, catalog: &str, db: &str) -> bool {
        if self.granted_global {
            return true;
        }

        if self
            .granted_databases
            .contains(&(catalog.to_string(), db.to_string()))
        {
            return true;
        }

        // if one of the tables in the database is granted, the database is also visible
        if self
            .extra_databases
            .contains(&(catalog.to_string(), db.to_string()))
        {
            return true;
        }

        false
    }

    pub fn check_table_visibility(&self, catalog: &str, database: &str, table: &str) -> bool {
        if self.granted_global {
            return true;
        }

        // if database is granted, all the tables in it are visible
        if self
            .granted_databases
            .contains(&(catalog.to_string(), database.to_string()))
        {
            return true;
        }

        if self.granted_tables.contains(&(
            catalog.to_string(),
            database.to_string(),
            table.to_string(),
        )) {
            return true;
        }

        false
    }
}
