ARG RUST_VERSION=1.94

## BUILD
FROM rust:${RUST_VERSION}-alpine AS build

WORKDIR /karon

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=./target \
    --mount=type=bind,source=./Cargo.toml,target=./Cargo.toml \
    --mount=type=bind,source=./Cargo.lock,target=./Cargo.lock \
    --mount=type=bind,source=./src,target=./src \
    --mount=type=bind,source=./templates,target=./templates \
    --mount=type=bind,source=./locales,target=./locales \
    --mount=type=bind,source=./migrations,target=./migrations \
    \
    cargo build --locked --release \
    && cp ./target/release/atlas .

## RUN
FROM alpine:latest

ARG UID=10001
ARG USER=karon

RUN adduser \
    --disabled-password \
    --no-create-home \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --uid "${UID}" \
    ${USER}

USER ${USER}

WORKDIR /karon
COPY --from=build /karon/karon .
COPY ./static /karon/static

EXPOSE ${KARON_PORT:-8080}

ENTRYPOINT [ "./karon" ]
