FROM node:20-trixie-slim@sha256:abfbe12cc943141a0c9e8c0a57d710df1dadd95d35e8662cc02958b284d1f35b

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /state \
    && chown node:node /state

COPY package.json package-lock.json ./
RUN npm ci --omit=dev

COPY *.js ./

USER node

EXPOSE 8080
CMD ["node", "server.js"]
