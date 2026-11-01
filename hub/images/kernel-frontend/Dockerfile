#  OpenSpoke フロントエンド (Nuxt 3 / Node.js)
#
#  二段で焼く。前段で依存を入れて建て、後段には出来たものだけを置く。
#  実行時に npm install を走らせないので、起動が速く、外に出ていかない。

FROM node:22-alpine AS build
WORKDIR /build

#  依存だけを先に入れる (ソースが変わっても、この層は使い回せる)
COPY package.json package-lock.json* ./
RUN npm install --no-audit --no-fund

COPY . .
RUN npm run build


FROM node:22-alpine AS runtime
WORKDIR /app

ENV NODE_ENV=production
ENV NITRO_PORT=3000
ENV NITRO_HOST=0.0.0.0
ENV TZ=Asia/Tokyo

#  nuxt build の出来上がりは .output だけで動く (node_modules を要さない)
COPY --from=build /build/.output ./.output

#  root で動かさない
USER node

EXPOSE 3000
CMD ["node", ".output/server/index.mjs"]
