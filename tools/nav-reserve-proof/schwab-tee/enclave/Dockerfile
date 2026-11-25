FROM python@sha256:ed86c82274b3c69b52fb5820f358f0bd7df0b603332063cb5c6e32bd220c3e6e AS build
ARG SOURCE_DATE_EPOCH
COPY requirements.lock /build/requirements.lock
RUN python -m pip install --no-cache-dir --no-compile --only-binary=:all: --require-hashes --target /build/packages -r /build/requirements.lock
COPY source /build/source
COPY libnsm.so aws-nitro-root.der aws-nitro-root.sha256 build-inputs.json /build/
COPY assemble.py /build/assemble.py
RUN python /build/assemble.py

FROM scratch
COPY --from=build /rootfs/ /
ENV PATH=/usr/local/bin
ENV LD_LIBRARY_PATH=/usr/local/lib
WORKDIR /app
ENTRYPOINT ["/usr/local/bin/python3.13", "-I", "-B", "-u", "-m", "pftl_schwab_tee.nitro_entrypoint"]
