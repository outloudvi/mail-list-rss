FROM rust as planner
WORKDIR /app

RUN cargo install cargo-chef
COPY . .
RUN cargo chef prepare  --recipe-path recipe.json

FROM rust as cacher
WORKDIR /app

RUN cargo install cargo-chef 
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM rust as builder
WORKDIR /app

COPY . .
COPY --from=cacher /app/target target
RUN cargo build --release 

FROM rust as runtime
WORKDIR /app
COPY --from=builder /app/target/release/mail-list-rss .
