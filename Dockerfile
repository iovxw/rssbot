FROM alpine as build

ARG LOCALE=zh

ENV RUSTFLAGS="-C target-feature=-crt-static" LOCALE=${LOCALE}
WORKDIR /usr/src/rssbot
COPY . .
RUN apk add --no-cache rustup openssl-dev build-base && rustup-init -y --default-toolchain nightly && source ${HOME}/.cargo/env && cargo build --release

FROM alpine

RUN apk add --no-cache ca-certificates openssl libgcc
ENTRYPOINT [ "/rssbot" ]

COPY --from=build /usr/src/rssbot/target/release/rssbot ./