FROM node:24-bookworm-slim AS build
WORKDIR /app
COPY apps/web/package*.json ./
RUN --mount=type=secret,id=build_ca \
    if [ -f /run/secrets/build_ca ]; then export NODE_EXTRA_CA_CERTS=/run/secrets/build_ca; fi; \
    npm ci
COPY apps/web/ ./
RUN npm run build && npm prune --omit=dev
FROM node:24-bookworm-slim
WORKDIR /app
ENV NODE_ENV=production HOST=0.0.0.0 PORT=3000
COPY --from=build --chown=node:node /app/build ./build
COPY --from=build --chown=node:node /app/node_modules ./node_modules
COPY --from=build --chown=node:node /app/package.json ./
USER node
EXPOSE 3000
HEALTHCHECK --interval=15s --timeout=3s CMD node -e "fetch('http://127.0.0.1:3000').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"
CMD ["node","build"]
