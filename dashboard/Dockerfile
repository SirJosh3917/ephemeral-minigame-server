FROM node:18 AS build
WORKDIR /src
RUN npm i -g pnpm

COPY package.json pnpm-lock.yaml ./
RUN pnpm install

COPY . .
RUN pnpm run build

# Not gonna copy `package.json`'s `type: module`
RUN mv ./dist/server.js ./dist/server_bak.mjs

# Enable CTRL-C when running the container interactively
RUN echo "process.on('SIGINT', process.exit);" >./dist/server_pre.mjs
RUN cat ./dist/server_bak.mjs ./dist/server_pre.mjs >./dist/server.mjs

FROM node:18-slim
WORKDIR /app

# This isn't as bad as NextJS at least... but...
# Seriously, what's up with these hot new JS frameworks that can't produce self-contained bundles?
RUN npm i undici@5.15.1

COPY --from=build /src/dist/ ./
ENTRYPOINT ["node", "server.mjs"]
