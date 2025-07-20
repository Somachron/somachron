FROM rust:1.88.0 AS builder

WORKDIR /usr/src/app
COPY . .

# Install build dependencies
RUN apt update
RUN apt install -y make curl make pkgconf clang git cmake \
    libssl-dev openssl \
    libavutil-dev libavformat-dev libavfilter-dev libavdevice-dev ffmpeg

# Build libheif
WORKDIR /usr/deps
RUN git clone https://github.com/strukturag/libheif.git

WORKDIR /usr/deps/libheif
RUN git checkout tags/v1.19.8
RUN mkdir build

WORKDIR /usr/deps/libheif/build
RUN cmake --preset=release ..
RUN make install

WORKDIR /usr/src/app
RUN rm -rf /usr/deps/libheif

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
# RUN RUSTFLAGS='-C target-feature=-crt-static' cargo install --locked --path .
RUN cargo install --locked --path .

EXPOSE 8080

# Run the binary
CMD [ "somachron" ]
