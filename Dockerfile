FROM rust:1.88-alpine3.22 AS builder

ENV LANG=C.UTF-8
ENV LC_ALL=C.UTF-8

# Install build dependencies
RUN apk update && apk add --no-cache \
    make \
    pkgconfig \
    clang \
    clang-dev \
    clang-static \
    git \
    cmake \
    curl \
    openssl-dev \
    openssl-libs-static \
    perl-image-exiftool \
    ffmpeg-dev \
    ffmpeg-libs \
    musl-dev \
    gcc \
    g++ \
    ninja \
    libjpeg-turbo-dev \
    libpng-dev \
    zlib-dev \
    x265-dev \
    x264-dev \
    dav1d-dev \
    libde265-dev

WORKDIR /dep
# Create exiftool symlink
RUN wget https://exiftool.org/Image-ExifTool-13.33.tar.gz && \
    tar -zxvf Image-ExifTool-13.33.tar.gz && \
    cd Image-ExifTool-13.33 && \
    perl Makefile.PL && make install

RUN git clone https://github.com/strukturag/libheif /dep/libheif
WORKDIR /dep/libheif
RUN git checkout tags/v1.19.8
WORKDIR /dep/libheif/build
RUN cmake --preset=release -DCMAKE_INSTALL_PREFIX=/usr/local ..
RUN make install -j$(nproc)
RUN ldconfig /usr/local/lib

# Set working directory
WORKDIR /app
COPY . .

# OpenSSL
# ENV OPENSSL_STATIC=1
# ENV OPENSSL_DIR=/usr
# # clang
# ENV LIBCLANG_PATH=/usr/lib
# ENV BINDGEN_EXTRA_CLANG_ARGS="-I/usr/include"
# ENV BINDGEN_STATIC_LIBCLANG=1

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
ENV RUSTFLAGS="-C target-feature=-crt-static"
RUN cargo install --path thumbnailer
RUN cargo install --path somachron

# Remove build artifacts to reduce image size
RUN rm -rf target

EXPOSE 8080

# Set memory allocator environment variable for better memory management
ENV MALLOC_MMAP_THRESHOLD_=65536

# Run the binary
CMD [ "somachron" ]
