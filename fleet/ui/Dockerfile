# syntax=docker/dockerfile:1.6

# ---- backend builder ----
FROM golang:1.22-alpine AS backend
WORKDIR /src
COPY backend/go.mod backend/go.sum* ./
COPY backend/ ./
# go.sum を Git に入れていないので、ビルド時に解決する。
# go mod tidy が依存をダウンロードし、 go.sum を生成してから go build へ。
RUN go mod tidy && \
    CGO_ENABLED=0 GOOS=linux go build -ldflags="-s -w" -o /out/fleet-ui ./...

# ---- runtime ----
FROM gcr.io/distroless/static:nonroot
WORKDIR /app
COPY --from=backend /out/fleet-ui /app/fleet-ui
COPY web/ /app/web/
USER nonroot:nonroot
EXPOSE 8080
ENTRYPOINT ["/app/fleet-ui"]
