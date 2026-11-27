# Reproducible verification container for Peel-Binius-Shielded (PFSH02).
FROM rust:1.95.0-bookworm

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
      curl ca-certificates python3 python3-pip python3-venv time \
    && rm -rf /var/lib/apt/lists/*

# Install Go 1.25.1 (linux/amd64)
RUN curl -fsSL https://go.dev/dl/go1.25.1.linux-amd64.tar.gz | tar -C /usr/local -xzf -
ENV PATH="/usr/local/go/bin:${PATH}"

# Install z3-solver for mechanized UNSAT proofs
RUN python3 -m venv /opt/venv \
    && /opt/venv/bin/pip install --no-cache-dir z3-solver==5.4.1.0
ENV PY_BIN="/opt/venv/bin/python"
ENV GO_BIN="/usr/local/go/bin/go"

WORKDIR /work/pft-binius-shielded-pq
COPY . .
RUN chmod +x ./verify.sh
CMD ["./verify.sh"]
