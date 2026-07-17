# ---- Planner Stage ----
FROM rust:1.93 AS chef

RUN cargo install cargo-chef

WORKDIR /app

FROM chef AS planner

COPY . .

RUN cargo chef prepare --recipe-path recipe.json

# ---- Cook Stage ----
FROM chef AS cook

ARG MODULE_NAME
ENV MODULE_NAME=${MODULE_NAME}

COPY --from=planner /app/recipe.json recipe.json

RUN cargo chef cook -p ${MODULE_NAME} --release --recipe-path recipe.json

# ---- Builder Stage ----
FROM rust:1.93 AS builder

ARG MODULE_NAME
ENV MODULE_NAME=${MODULE_NAME}

WORKDIR /app
COPY --from=cook /app/target target
COPY --from=cook /usr/local/cargo /usr/local/cargo
COPY . .

RUN cargo build -p ${MODULE_NAME} --release && \
  cp /app/target/release/$MODULE_NAME /app/target/release/application && \
  mkdir -p /app/${MODULE_NAME}/config && \
  cp -R /app/$MODULE_NAME/config /app/target/

# ---- Final Stage ----
FROM gcr.io/distroless/cc-debian13 AS runtime
ARG MODULE_NAME
ENV MODULE_NAME=${MODULE_NAME}
WORKDIR /app

COPY --from=builder /app/target/release/application .
COPY --from=builder /app/target/config ./config/

CMD ["./application"]
