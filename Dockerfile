FROM ubuntu:22.04 AS builder

ENV DEBIAN_FRONTEND=noninteractive
ENV LANG=C.UTF-8
ENV LC_ALL=C.UTF-8
ENV SHELL=/bin/bash

# Install build dependencies
RUN apt update
RUN apt install -y software-properties-common curl
RUN add-apt-repository -y ppa:strukturag/libheif
RUN apt install -y make curl make pkgconf clang git cmake \
    libssl-dev openssl \
    libimage-exiftool-perl \
    libavutil-dev libavformat-dev libavfilter-dev libavdevice-dev ffmpeg

RUN apt-get install -y libheif1 libheif-dev libavutil-dev libavformat-dev libavfilter-dev libavdevice-dev libimage-exiftool-perl

# Clean up
RUN apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Install Rust as root
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Set working directory
WORKDIR /app
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

# Remove build artifacts to reduce image size
RUN rm -rf target

EXPOSE 8080

# Run the binary
CMD [ "somachron" ]
