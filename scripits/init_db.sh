set -x
set -eo pipefail

#检查psql 和 sqlx 是否安装
if ! [ -x "$(command -v psql)" ]; then
    echo >&2 "Error: psql is not installed."
    exit 1
fi

if ! [ -x "$(command -v sqlx)" ];then
    echo >&2 "Error: sqlx is not installed."
    echo >&2 "Use:"
    echo >&2 "cargo install --version=0.6.0 sqlx-cli --no-default-features --features postgres"
    echo >&2 "to install it."
    exit 1
fi
#未设置则默认postgres
DB_USER=${POSTGRES_USER:=postgres}

#未设置密码则默认为password
DB_PASSWORD="${POSTGRES_PASSWORD:=password}"

#未设置数据库名称则默认为newsletter
DB_NAME="${POSTGRES_DB:=newsletter}"

#未设置端口则默认为5432
DB_PORT="${POSTGRES_PORT:=5432}"

#定义docker容器的名称
CONTAINER_NAME="newsletter_postgres"

#使用docker启动postgres
if [[ -z "$SKIP_DOCKER" ]]
then
    # 幂等性：清理旧容器
    docker rm -f "${CONTAINER_NAME}" || true

    docker run \
        --name "${CONTAINER_NAME}" \
        -e POSTGRES_USER=${DB_USER} \
        -e POSTGRES_PASSWORD=${DB_PASSWORD} \
        -e POSTGRES_DB=${DB_NAME} \
        -p "${DB_PORT}":5432 \
        -d postgres \
        postgres -N 1000
fi
#保持对 postgres的轮询，直到它准备好
export PGPASSWORD="${DB_PASSWORD}"
until psql -h "localhost" -U "${DB_USER}" -p "${DB_PORT}" -d "postgres" -c '\q';
do >&2 echo "postgres is still unavailable - sleeping"
   sleep 1
done

>&2 echo "postgres is up and running on port ${DB_PORT}!"

export DATABASE_URL=postgres://${DB_USER}:${DB_PASSWORD}@localhost:${DB_PORT}/${DB_NAME}
sqlx database create
sqlx migrate run

>&2 echo "Postgres has been migrated, ready to go!"
