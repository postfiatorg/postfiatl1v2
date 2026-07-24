# syntax=docker/dockerfile:1.7

# The base is pinned to the linux/amd64 manifest. The Bitcoin Core artifact is
# the reproducible release binary published by bitcoincore.org and its digest
# is taken from the release's signed SHA256SUMS file.
FROM debian:bookworm-slim@sha256:63a496b5d3b99214b39f5ed70eb71a61e590a77979c79cbee4faf991f8c0783e

ARG BITCOIN_VERSION=31.1
ARG BITCOIN_ARCHIVE_SHA256=b80d9c3e04da78fb6f0569685673418cf686fadba9042d926d13fb87ff503f9e

ADD --checksum=sha256:b80d9c3e04da78fb6f0569685673418cf686fadba9042d926d13fb87ff503f9e \
    https://bitcoincore.org/bin/bitcoin-core-31.1/bitcoin-31.1-x86_64-linux-gnu.tar.gz \
    /tmp/bitcoin-core.tar.gz

RUN set -eux; \
    echo "${BITCOIN_ARCHIVE_SHA256}  /tmp/bitcoin-core.tar.gz" | sha256sum -c -; \
    tar -xzf /tmp/bitcoin-core.tar.gz -C /tmp; \
    install -m 0755 "/tmp/bitcoin-${BITCOIN_VERSION}/bin/bitcoind" /usr/local/bin/bitcoind; \
    install -m 0755 "/tmp/bitcoin-${BITCOIN_VERSION}/bin/bitcoin-cli" /usr/local/bin/bitcoin-cli; \
    groupadd --gid 101 bitcoin; \
    useradd --uid 101 --gid 101 --home-dir /bitcoin --no-create-home --shell /usr/sbin/nologin bitcoin; \
    install -d -o bitcoin -g bitcoin -m 0700 /bitcoin; \
    rm -rf /tmp/bitcoin-core.tar.gz "/tmp/bitcoin-${BITCOIN_VERSION}"

LABEL org.opencontainers.image.title="PFTL synthetic Bitcoin Core regtest"
LABEL org.opencontainers.image.version="31.1"
LABEL org.opencontainers.image.source="https://bitcoincore.org/bin/bitcoin-core-31.1/"
LABEL org.opencontainers.image.revision="b80d9c3e04da78fb6f0569685673418cf686fadba9042d926d13fb87ff503f9e"

USER bitcoin
VOLUME ["/bitcoin"]
ENTRYPOINT ["/usr/local/bin/bitcoind"]
