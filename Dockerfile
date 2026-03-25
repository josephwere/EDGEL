FROM rust:1.86-bookworm AS builder
WORKDIR /app
COPY . /app
RUN cargo build --release -p goldedge-browser

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . /app
COPY --from=builder /app/target/release/goldedge-browser /app/bin/goldedge-browser
ENV EDGEL_HOST=0.0.0.0
ENV PORT=4040
EXPOSE 4040
CMD ["/app/bin/goldedge-browser"]
