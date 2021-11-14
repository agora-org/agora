# 1: Build the exe
FROM rust:1.56 as builder

# WORKDIR /usr/src

# RUN USER=root cargo new agora

# COPY Cargo.toml Cargo.lock /usr/src/agora/

WORKDIR /usr/src/agora

# Prepare for static linking
# RUN apt-get update && \
#     apt-get dist-upgrade -y && \
#     apt-get install -y musl-tools && \
#     rustup target add x86_64-unknown-linux-musl

# Download and compile Rust dependencies (and store as a separate Docker layer)

# copy local dependencies
# COPY ./agora-lnd-client ./agora-lnd-client
# COPY ./lnd-test-context ./lnd-test-context
# COPY ./bin ./bin
# build dependencies
# RUN cargo build --release

# Build the exe using the actual source code
COPY . ./
RUN cargo build --release

# Copy the exe and extra files ("static") to an empty Docker image
FROM ubuntu as final

ARG APP=/usr/src/app
ENV APP_USER=agorauser

RUN groupadd $APP_USER \
    && useradd -g $APP_USER $APP_USER \
    && mkdir -p ${APP}

COPY --from=builder /usr/src/agora/target/release/agora ${APP}/bin/agora
COPY static .
# copy lnd auth files
COPY ./.lnd ${APP}/.lnd
# copy in files
COPY ./files ${APP}/files

RUN chown -R $APP_USER:$APP_USER ${APP}

# run
WORKDIR ${APP}
COPY ./entrypoint.sh ./
COPY .env .
RUN chmod a+x entrypoint.sh
USER $APP_USER
CMD ["./entrypoint.sh"]