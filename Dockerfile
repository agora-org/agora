# 1: Build the exe
FROM rust:1.56 as builder

# WORKDIR /usr/src

# RUN USER=root cargo new agora

# COPY Cargo.toml Cargo.lock /usr/src/agora/

WORKDIR /usr/src/agora

COPY . ./
RUN cargo build --release

FROM ubuntu as final

ARG APP=/usr/src/app
ENV APP_USER=agora

RUN useradd --system ${APP_USER}

COPY --from=builder /usr/src/agora/target/release/agora ${APP}/bin/agora
COPY static .
COPY ./.lnd ${APP}/.lnd
COPY ./files ${APP}/files

RUN chown -R $APP_USER:$APP_USER ${APP}

WORKDIR ${APP}
COPY ./entrypoint.sh ./
COPY .env .
RUN chmod a+x entrypoint.sh
USER $APP_USER
CMD ["./entrypoint.sh"]
