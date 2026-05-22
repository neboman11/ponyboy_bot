# 1. This tells docker to use the Rust official image
FROM rust:1.88

# 2. Copy the files in your machine to the Docker image
WORKDIR /app
COPY src ./src
COPY Cargo.toml ./
COPY Cargo.lock ./

# Build your program for release
RUN cargo build --release

VOLUME /data

EXPOSE 8081

# Run the binary
CMD ["./target/release/ponyboy_bot"]
