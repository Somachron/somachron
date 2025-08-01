FROM rust:1.88-bookworm AS builder

ENV LANG=C.UTF-8
ENV LC_ALL=C.UTF-8

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    clang \
    git \
    cmake \
    curl \
    libssl-dev \
    ninja-build \
    && rm -rf /var/lib/apt/lists/*

USER root

WORKDIR /app/surrealdb
RUN git clone https://github.com/surrealdb/surrealdb.git .
RUN git checkout tags/v2.3.7

RUN cargo install --path .
RUN rm -rf target

EXPOSE 8000

VOLUME /data /logs
ENV SURREAL_BIND="0.0.0.0:8000"
ENV MALLOC_MMAP_THRESHOLD_=65536

CMD [ "surrealdb" ]
