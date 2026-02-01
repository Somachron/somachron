FROM rust:1.93-alpine3.22 AS builder

ENV LANG=C.UTF-8
ENV LC_ALL=C.UTF-8

WORKDIR /app
COPY . .

RUN cargo install --path somachron

FROM alpine:3.22

COPY --from=builder /usr/local/cargo/bin/somachron /usr/local/bin/somachron
COPY --from=builder /usr/src/app/lib-migrations/migrations /usr/local/bin/migrations

EXPOSE 8080

CMD [ "somachron" ]
