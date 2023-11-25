#!/usr/bin/env bash

CURDIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
. "$CURDIR"/../../../shell_env.sh

export TEST_USER_NAME="u1"
export TEST_USER_PASSWORD="password"
export TEST_USER_CONNECT="bendsql --user=u1 --password=password --host=${QUERY_MYSQL_HANDLER_HOST} --port ${QUERY_HTTP_HANDLER_PORT}"

echo "drop table if exists test_table;" | $BENDSQL_CLIENT_CONNECT
echo "drop user if exists u1;" | $BENDSQL_CLIENT_CONNECT
echo "drop STAGE if exists s2;" | $BENDSQL_CLIENT_CONNECT
echo "CREATE STAGE s2;" | $BENDSQL_CLIENT_CONNECT

echo "CREATE TABLE test_table (
    id INTEGER,
    name VARCHAR,
    age INT
);" | $BENDSQL_CLIENT_CONNECT

for i in `seq 1 10`;do
    echo "insert into test_table (id,name,age) values(1,'2',3), (4, '5', 6);" | $BENDSQL_CLIENT_CONNECT
done

STAGE_DIR=/tmp/copy_into_stage2

rm -rf "$STAGE_DIR"
echo "create stage s1 url = 'fs:///$STAGE_DIR/' FILE_FORMAT = (type = CSV)" | $BENDSQL_CLIENT_CONNECT

echo "create user u1 identified by 'password';" | $BENDSQL_CLIENT_CONNECT
echo "grant insert on default.test_table to u1;" | $BENDSQL_CLIENT_CONNECT
echo "==== check internal stage write priv ==="
echo "copy into @s2 from test_table FILE_FORMAT = (type = CSV);" | $TEST_USER_CONNECT
echo "grant Write on stage s2 to 'u1'" | $BENDSQL_CLIENT_CONNECT
echo "copy into @s2 from test_table FILE_FORMAT = (type = CSV);" | $TEST_USER_CONNECT
echo "grant select on default.test_table to u1;" | $BENDSQL_CLIENT_CONNECT
echo "copy into @s2 from test_table FILE_FORMAT = (type = CSV);" | $TEST_USER_CONNECT
echo "list @s2;" | $BENDSQL_CLIENT_CONNECT | wc -l | sed 's/ //g'

echo "==== check external stage priv ==="
echo "copy into @s1/csv/ from test_table FILE_FORMAT = (type = CSV);" | $TEST_USER_CONNECT
echo "grant write on stage s1 to 'u1'" | $BENDSQL_CLIENT_CONNECT
echo "copy into @s1/csv/ from test_table FILE_FORMAT = (type = CSV);" | $TEST_USER_CONNECT
echo "copy into test_table from @s1/csv/ FILE_FORMAT = (type = CSV skip_header = 0) force=true;" | $TEST_USER_CONNECT
echo "grant read on stage s1 to 'u1'" | $BENDSQL_CLIENT_CONNECT
echo "copy into test_table from @s1/csv/ FILE_FORMAT = (type = CSV skip_header = 0) force=true;" | $TEST_USER_CONNECT

echo "==== check internal stage read priv ==="
echo "truncate table test_table;" | $BENDSQL_CLIENT_CONNECT
echo "copy into test_table from @s2 FILE_FORMAT = (type = CSV skip_header = 0) force=true;" | $TEST_USER_CONNECT
echo "grant Read on stage s2 to 'u1'" | $BENDSQL_CLIENT_CONNECT
echo "copy into test_table from @s2 FILE_FORMAT = (type = CSV skip_header = 0) force=true;" | $TEST_USER_CONNECT

echo "remove @s2;" | $BENDSQL_CLIENT_CONNECT
echo "remove @s1;" | $BENDSQL_CLIENT_CONNECT
echo "drop STAGE s2;" | $BENDSQL_CLIENT_CONNECT
echo "drop STAGE s1;" | $BENDSQL_CLIENT_CONNECT
echo "drop table test_table;" | $BENDSQL_CLIENT_CONNECT

echo "=== check presign ==="
echo "drop stage if exists presign_stage" | $BENDSQL_CLIENT_CONNECT

echo "CREATE STAGE presign_stage;" | $BENDSQL_CLIENT_CONNECT

# Most arguments is the same with previous, except:
# -X PUT: Specify the http method
echo "grant Write, Read on stage presign_stage to 'u1'" | $BENDSQL_CLIENT_CONNECT
echo "revoke Write on stage presign_stage from 'u1'" | $BENDSQL_CLIENT_CONNECT
curl -s -w "%{http_code}\n" -X PUT -o /dev/null -H Content-Type:application/octet-stream "`echo "PRESIGN UPLOAD @presign_stage/hello_world.txt CONTENT_TYPE='application/octet-stream'" | $TEST_USER_CONNECT`"

echo "revoke Read on stage presign_stage from 'u1'" | $BENDSQL_CLIENT_CONNECT
curl -s -w "%{http_code}\n" -o /dev/null "`echo "PRESIGN @presign_stage/hello_word.txt" | $TEST_USER_CONNECT`"

## Drop table.
echo "drop stage if exists presign_stage" | $BENDSQL_CLIENT_CONNECT
echo "drop user u1"  | $BENDSQL_CLIENT_CONNECT
