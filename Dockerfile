#######################################
#构建阶段

##使用Rust稳定版本作为基础镜像,里面装好了Rust 1.85 和 Debian系统
FROM rust:1.85-slim-bookworm AS builder

#把工作目录切换到`app`,相当于 mkdir /app && cd /app
#`app`文件夹将由Docker创建,并且切换到该目录下面
WORKDIR /app

#为链接配置安装所需要的系统依赖, RUN 执行指令
RUN apt update && apt install lld clang -y

#将工作环境中的所有文件复制到Docker镜像中,第一个是当前目录，第二是容器内的/app目录
COPY . .

#设置环境变量
ENV SQLX_OFFLINE=true

#RUN 执行命令，这里是生成最终的可执行文件
RUN cargo build --release

############################################

#运行时阶段
FROM debian:bookworm-slim AS runtime

WORKDIR /app

#安装 OpenSSL
#安装 ca-certificates——在建立HTTPS连接时，需要验证 TLS 证书
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl ca-certificates \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*


#从构建环境中复制已编译的二进制文件到运行时环境中
COPY --from=builder /app/target/release/zero2prod zero2prod

#在运行时需要配置文件
COPY configuration configuration

#设置APP_ENVIRONMENT 环境变量，从而生成生产配置
ENV APP_ENVIRONMENT production

#当执行`docker run`时，启动二进制文件
ENTRYPOINT ["./zero2prod"]
