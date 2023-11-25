#!/usr/bin/env bash

CURDIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
. "$CURDIR"/../../../shell_env.sh

curl -s -u 'root:' -XPOST "http://localhost:${QUERY_CLICKHOUSE_HTTP_HANDLER_PORT}/" -d "drop table if exists a"
curl -s -u 'root:' -XPOST "http://localhost:${QUERY_CLICKHOUSE_HTTP_HANDLER_PORT}/" -d "create table a ( a int not null, b string not null, c int not null)"

cat << EOF > /tmp/databend_test_csv_error.txt
insert into a(a,b) format CSV 1,"Hello",1

EOF
curl -s -u 'root:' -XPOST "http://localhost:${QUERY_CLICKHOUSE_HTTP_HANDLER_PORT}/" --data-binary @/tmp/databend_test_csv_error.txt
curl -s -u 'root:' -XPOST "http://localhost:${QUERY_CLICKHOUSE_HTTP_HANDLER_PORT}/" -d "drop table if exists a"