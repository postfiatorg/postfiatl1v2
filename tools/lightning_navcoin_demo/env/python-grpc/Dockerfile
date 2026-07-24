# syntax=docker/dockerfile:1.7

FROM python:3.12.13-slim-bookworm@sha256:72d3d75f2639ab82b34b29390ad3d6e0827c775befee94edda8e9976818f488d AS generator

COPY requirements-build.txt /tmp/requirements-build.txt
RUN python -m pip install \
    --no-cache-dir \
    --only-binary=:all: \
    --require-hashes \
    -r /tmp/requirements-build.txt

# Generated modules are tied to the same v0.20.1-beta source tag as the pinned
# LND runtime. The archive digest is intentionally fixed.
ADD --checksum=sha256:db479a6d4cf7bd00817f8ce9bad2f57d81933b0b408364dc3f9c1e94682a27d3 \
    https://github.com/lightningnetwork/lnd/archive/refs/tags/v0.20.1-beta.tar.gz \
    /tmp/lnd.tar.gz

RUN set -eux; \
    echo "db479a6d4cf7bd00817f8ce9bad2f57d81933b0b408364dc3f9c1e94682a27d3  /tmp/lnd.tar.gz" | sha256sum -c -; \
    mkdir -p /tmp/lnd-source /tmp/proto /generated; \
    tar -xzf /tmp/lnd.tar.gz -C /tmp/lnd-source --strip-components=1; \
    cp /tmp/lnd-source/lnrpc/lightning.proto /tmp/proto/lightning.proto; \
    cp /tmp/lnd-source/lnrpc/routerrpc/router.proto /tmp/proto/router.proto; \
    python -m grpc_tools.protoc \
      -I/tmp/proto \
      --python_out=/generated \
      --grpc_python_out=/generated \
      /tmp/proto/lightning.proto \
      /tmp/proto/router.proto; \
    touch /generated/__init__.py; \
    sha256sum \
      /tmp/proto/lightning.proto \
      /tmp/proto/router.proto \
      /generated/lightning_pb2.py \
      /generated/lightning_pb2_grpc.py \
      /generated/router_pb2.py \
      /generated/router_pb2_grpc.py \
      > /generated/LND_PROTO_MANIFEST.sha256

FROM python:3.12.13-slim-bookworm@sha256:72d3d75f2639ab82b34b29390ad3d6e0827c775befee94edda8e9976818f488d

COPY requirements-runtime.txt /tmp/requirements-runtime.txt
RUN python -m pip install \
    --no-cache-dir \
    --only-binary=:all: \
    --require-hashes \
    -r /tmp/requirements-runtime.txt

COPY --from=generator /generated/ /opt/pftl-lnd-grpc/
COPY pftl_lnd_grpc.py smoke.py /opt/pftl-lnd-grpc/

ENV PYTHONPATH=/opt/pftl-lnd-grpc
ENV PYTHONDONTWRITEBYTECODE=1
ENV PYTHONUNBUFFERED=1

LABEL org.opencontainers.image.title="PFTL synthetic LND Python gRPC runtime"
LABEL org.opencontainers.image.version="v0.20.1-beta"
LABEL org.opencontainers.image.source="https://github.com/lightningnetwork/lnd/tree/v0.20.1-beta/lnrpc"
LABEL org.opencontainers.image.revision="db479a6d4cf7bd00817f8ce9bad2f57d81933b0b408364dc3f9c1e94682a27d3"

ENTRYPOINT ["python3"]
