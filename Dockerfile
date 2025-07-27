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
    libexif-dev \
    libimage-exiftool-perl \
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

# Build and install libheif
WORKDIR /tmp/libheif
RUN git clone https://github.com/strukturag/libheif . && \
    git checkout tags/v1.19.8 && \
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
RUN cargo install --path somachron && \
    cargo install --path thumbnailer

# Runtime stage
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies only
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libexif12 \
    libimage-exiftool-perl \
    libavdevice59 \
    libjpeg62-turbo \
    libpng16-16 \
    zlib1g \
    libx265-199 \
    libx264-164 \
    libdav1d6 \
    libde265-0 \
    libaom3 \
    libsvtav1enc1 \
    && rm -rf /var/lib/apt/lists/*

# Copy libheif from builder
COPY --from=builder /usr/local/lib/libheif* /usr/local/lib/
RUN ldconfig

# Copy binaries from builder
COPY --from=builder /usr/local/cargo/bin/somachron /usr/local/bin/
COPY --from=builder /usr/local/cargo/bin/thumbnailer /usr/local/bin/

EXPOSE 8080

# Set memory allocator environment variable for better memory management
ENV MALLOC_MMAP_THRESHOLD_=65536

# Run the binary
CMD [ "somachron" ]
