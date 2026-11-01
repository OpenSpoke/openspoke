#  sysml-validator (OpenSpoke の知識ベースに書く SysML v2 本文を調べるサービス)
#
#  二段で焼く。前段で Rust を組み立て、後段には出来上がった 1 本だけを置く。
#  構文チェッカー sysml2 と バリデーター mercurio-sysml は Cargo.toml で版を固定している (=0.3.1、=0.86.0)。
#  間接の依存が変わって壊れた場合は、下の cargo test が落ちてイメージは作られない。

FROM rust:1-bookworm AS build
WORKDIR /build
COPY Cargo.toml ./
COPY src ./src
COPY tests ./tests
#  わざと壊した入力を確実に落とせるかを、組み立てのたびに確かめる (落とせなければイメージを作らない)
RUN cargo test --release && cargo build --release

FROM debian:bookworm-slim AS runtime
RUN useradd --system --uid 10001 --no-create-home validator
COPY --from=build /build/target/release/sysml-validator /usr/local/bin/sysml-validator
USER 10001
ENV LISTEN_ADDR=0.0.0.0:8080
EXPOSE 8080
CMD ["/usr/local/bin/sysml-validator"]
