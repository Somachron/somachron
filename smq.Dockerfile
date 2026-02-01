FROM rust:1.93-bookworm AS builder

ENV LANG=C.UTF-8
ENV LC_ALL=C.UTF-8

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    perl \
    make \
    tar \
    pkg-config \
    clang \
    git \
    cmake \
    curl \
    libssl-dev \
    libexif-dev \
    libavdevice-dev \
    ninja-build \
    libjpeg62-turbo-dev \
    libpng-dev \
    zlib1g-dev \
    libx265-dev \
    libx264-dev \
    libdav1d-dev \
    libde265-dev \
    libaom-dev \
    libsvtav1-dev \
    wget \
    && rm -rf /var/lib/apt/lists/*

# Install exiftool
WORKDIR /tmp/exiftool
RUN git clone https://github.com/exiftool/exiftool.git . && \
    git checkout tags/13.33 && \
    perl Makefile.PL && \
    make install

# Build and install libheif
WORKDIR /tmp/libheif
RUN git clone https://github.com/strukturag/libheif . && \
    git checkout tags/v1.21.2 && \
    mkdir build && cd build && \
    cmake --preset=release \
    -DCMAKE_INSTALL_PREFIX=/usr/local \
    -DWITH_X265=ON \
    -DWITH_AOM=ON \
    -DWITH_DAV1D=ON \
    -DWITH_LIBDE265=ON \
    -DWITH_SvtEnc=ON \
    .. && \
    make install -j$(nproc) && \
    ldconfig

WORKDIR /app
COPY . .

RUN cargo install --path somachron-media-queue

EXPOSE 8080

ENV MALLOC_MMAP_THRESHOLD_=65536

CMD [ "somachron-media-queue" ]
