# syntax=docker/dockerfile:1.7
# Build from the monorepo root:  docker build -f apps/backend/Dockerfile -t titi-relay .
FROM node:24-alpine AS build
RUN corepack enable && corepack prepare pnpm@10.18.0 --activate
WORKDIR /repo
COPY package.json pnpm-workspace.yaml pnpm-lock.yaml ./
COPY packages/protocol/package.json packages/protocol/
COPY apps/backend/package.json apps/backend/
RUN pnpm install --frozen-lockfile --filter @titi/backend...
COPY packages/protocol packages/protocol
COPY apps/backend apps/backend
RUN pnpm --filter @titi/backend build \
 && pnpm --filter @titi/backend deploy --prod --legacy /out

FROM gcr.io/distroless/nodejs24-debian12:nonroot
WORKDIR /app
COPY --from=build /out/dist ./dist
COPY --from=build /out/node_modules ./node_modules
COPY --from=build /out/package.json ./package.json
ENV NODE_ENV=production PORT=8080
EXPOSE 8080
CMD ["dist/index.js"]
