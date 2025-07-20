FROM rust:1.88.0-alpine3.21 as builder

WORKDIR /usr/src/app
COPY . .

# Install build dependencies
RUN apk add --no-cache musl-dev perl-utils make curl make pkgconf openssl-dev openssl-libs-static libheif libheif-dev ffmpeg ffmpeg-dev clang17-libclang

ARG R2_ACCOUNT_ID
ARG R2_BUCKET
ARG R2_ACCESS_KEY
ARG R2_SECRET_KEY

ARG GOOGLE_CLIENT_ID
ARG GOOGLE_CLIENT_SECRET
ARG GOOGLE_REDIRECT_URI

ARG DATABASE_URL
ARG VOLUME_PATH

# Build the application
RUN RUSTFLAGS='-C target-feature=-crt-static' cargo install --locked --path .

# Start a new, final image
FROM alpine:3.21

# Copy the binary from the build stage
COPY --from=builder /usr/local/cargo/bin/somachron /usr/local/bin/somachron
COPY --from=builder /usr/src/app/lib-migrations/migrations /usr/local/bin/migrations

EXPOSE 8080

# Run the binary
CMD [ "somachron" ]
