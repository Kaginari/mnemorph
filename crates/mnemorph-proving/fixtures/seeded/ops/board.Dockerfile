FROM node:22 AS build
WORKDIR /app
COPY . .
RUN corepack enable && pnpm install && pnpm --filter @mnemorph/board build

FROM node:22-slim AS runtime
COPY --from=build /app /app
EXPOSE 8080
CMD ["node", "/app/packages/board/dist/server.js"]
