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

use common_base::base::tokio;
use common_exception::Result;
use common_expression::types::DataType;
use common_grpc::RpcClientConf;
use common_meta_app::principal::UserDefinedFunction;
use common_users::UserApiProvider;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_user_lambda_udf() -> Result<()> {
    let conf = RpcClientConf::default();
    let user_mgr = UserApiProvider::try_create_simple(conf).await?;

    let tenant = "test";
    let description = "this is a description";
    let isempty = "isempty";
    let isnotempty = "isnotempty";
    let if_not_exists = false;

    // add isempty.
    let isempty_udf = UserDefinedFunction::create_lambda_udf(
        isempty,
        vec!["p".to_string()],
        "is_null(p)",
        description,
    );
    user_mgr
        .add_udf(tenant, isempty_udf.clone(), if_not_exists)
        .await?;

    // add isnotempty.
    let isnotempty_udf = UserDefinedFunction::create_lambda_udf(
        isnotempty,
        vec!["p".to_string()],
        "not(isempty(p))",
        description,
    );
    user_mgr
        .add_udf(tenant, isnotempty_udf.clone(), if_not_exists)
        .await?;

    // get all.
    {
        let udfs = user_mgr.get_udfs(tenant).await?;
        assert_eq!(udfs, vec![isempty_udf.clone(), isnotempty_udf.clone()]);
    }

    // get.
    {
        let udf = user_mgr.get_udf(tenant, isempty).await?;
        assert_eq!(udf, isempty_udf.clone());
    }

    // drop.
    {
        user_mgr.drop_udf(tenant, isnotempty, false).await?;
        let udfs = user_mgr.get_udfs(tenant).await?;
        assert_eq!(udfs, vec![isempty_udf]);
    }

    // repeat drop same one not with if exist.
    {
        let res = user_mgr.drop_udf(tenant, isnotempty, false).await;
        assert!(res.is_err());
    }

    // repeat drop same one with if exist.
    {
        let res = user_mgr.drop_udf(tenant, isnotempty, true).await;
        assert!(res.is_ok());
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_user_udf_server() -> Result<()> {
    let conf = RpcClientConf::default();
    let user_mgr = UserApiProvider::try_create_simple(conf).await?;

    let tenant = "test";
    let address = "http://127.0.0.1:8888";
    let arg_types = vec![DataType::String];
    let return_type = DataType::Boolean;
    let description = "this is a description";
    let isempty = "isempty";
    let isnotempty = "isnotempty";
    let if_not_exists = false;

    // add isempty.
    let isempty_udf = UserDefinedFunction::create_udf_server(
        isempty,
        address,
        isempty,
        "python",
        arg_types.clone(),
        return_type.clone(),
        description,
    );
    user_mgr
        .add_udf(tenant, isempty_udf.clone(), if_not_exists)
        .await?;

    // add isnotempty.
    let isnotempty_udf = UserDefinedFunction::create_udf_server(
        isnotempty,
        address,
        isnotempty,
        "python",
        arg_types.clone(),
        return_type.clone(),
        description,
    );
    user_mgr
        .add_udf(tenant, isnotempty_udf.clone(), if_not_exists)
        .await?;

    // get all.
    {
        let udfs = user_mgr.get_udfs(tenant).await?;
        assert_eq!(udfs, vec![isempty_udf.clone(), isnotempty_udf.clone()]);
    }

    // get.
    {
        let udf = user_mgr.get_udf(tenant, isempty).await?;
        assert_eq!(udf, isempty_udf.clone());
    }

    // drop.
    {
        user_mgr.drop_udf(tenant, isnotempty, false).await?;
        let udfs = user_mgr.get_udfs(tenant).await?;
        assert_eq!(udfs, vec![isempty_udf]);
    }

    // repeat drop same one not with if exist.
    {
        let res = user_mgr.drop_udf(tenant, isnotempty, false).await;
        assert!(res.is_err());
    }

    // repeat drop same one with if exist.
    {
        let res = user_mgr.drop_udf(tenant, isnotempty, true).await;
        assert!(res.is_ok());
    }

    Ok(())
}
