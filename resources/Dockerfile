FROM rust:alpine as toolchain

RUN useradd -m runner -d /runner
RUN usermod -p '!!' root # Disable all passwords for root

RUN apk add valgrind
RUN cargo install --version 0.14.0 iai-callgrind-runner

USER runner
ENV USER=runner
ENV PATH=/runner/.cargo/bin:$PATH
WORKDIR /runner

FROM toolchain as sources

COPY --chown=runner runner/ /runner
RUN cargo fetch

FROM toolchain as prepare-worker

WORKDIR /worker

COPY --chown=runner worker/ /worker
RUN cargo install --locked --path .

FROM sources

RUN cargo build --release --bench bench
RUN rm src/*.rs

COPY --from=prepare-worker /worker/.cargo/bin/worker /runner/.cargo/bin/worker

ENTRYPOINT [ "worker" ]