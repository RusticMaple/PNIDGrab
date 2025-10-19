FROM rust:alpine AS builder

WORKDIR /build

COPY . .

RUN apk add patchelf gcc musl-dev openssl-dev openssl-libs-static glib-dev glib-static gdk-pixbuf-dev gtk4.0-dev libadwaita-dev 

RUN OPENSSL_STATIC=1 RUSTFLAGS='-C target-feature=-crt-static' cargo build --release


FROM alpine AS output

RUN apk add --no-cache glib openssl gdk-pixbuf gtk4.0 libadwaita mesa-gles zenity patchelf

COPY --from=builder /build/target/release/pnidgrab /pnidgrab

RUN patchelf --set-interpreter "./lib/ld-musl-x86_64.so.1" /pnidgrab
RUN patchelf --set-interpreter "./lib/ld-musl-x86_64.so.1" /usr/bin/zenity

