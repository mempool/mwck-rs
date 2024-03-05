FROM rust:latest
FROM mempool/mempool as mempool
LABEL org.opencontainers.image.source="https://github.com/bitcoincore-dev/mempool_space"
LABEL org.opencontainers.image.description="mempool-space"
RUN touch updated
RUN echo $(date +%s) > updated
RUN apt-get update
RUN apt-get install -y bash cmake git libssl-dev make tcl-dev
RUN git clone --branch master --depth 1 https://github.com/bitcoincore-dev/mempool_space.git
WORKDIR /tmp
RUN git clone --recurse-submodules -j4 --branch master --depth 10 https://github.com/bitcoincore-dev/mempool_space.git
WORKDIR /tmp/mempool
RUN make detect
RUN make cargo-i
ENV SUDO=sudo
RUN cargo install gnostr-bins --force
RUN install ./serve /usr/local/bin || true
ENV PATH=$PATH:/usr/bin/systemctl
RUN ps -p 1 -o comm=
EXPOSE 80 6102 8080 ${PORT}
VOLUME /src
FROM mempool as mempool-docker

