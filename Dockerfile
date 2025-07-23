FROM rust:1.88.0 AS builder

# Install build dependencies
RUN apt update
RUN apt install -y make curl make pkgconf clang git cmake \
    libssl-dev openssl \
    libimage-exiftool-perl \
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

# Remove build
WORKDIR /usr/src/app
RUN rm -rf /usr/deps/libheif
COPY . .

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
RUN cargo install --path somachron
RUN cargo install --path thumbnailer

# Remove build
RUN rm -rf target

EXPOSE 8080

ENV JEMALLOC_SYS_WITH_MALLOC_CONF="background_thread:true,narenas:1,tcache:false,dirty_decay_ms:0,muzzy_decay_ms:0,abort_conf:true"
ENV MALLOC_CONF="background_thread:true,narenas:1,tcache:false,dirty_decay_ms:0,muzzy_decay_ms:0,abort_conf:true"

# Run the binary
CMD [ "somachron" ]
