FROM rust:1.93-alpine3.22 AS builder

WORKDIR /app
COPY . .

RUN apk add --no-cache musl-dev perl-utils make curl make pkgconf openssl-dev openssl-libs-static

RUN cargo install --path somarift

FROM alpine:3.22

COPY --from=builder /usr/local/cargo/bin/somarift /usr/local/bin/somarift
COPY --from=builder /app/lib-migrations/migrations /usr/local/bin/migrations

EXPOSE 8080

CMD [ "somarift" ]
