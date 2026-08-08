# tunnel-client: OpenSpoke spoke-side reverse tunnel client
FROM golang:1.22-bookworm AS build

RUN apt-get update && apt-get install -y --no-install-recommends \
      protobuf-compiler ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY go.mod ./
COPY *.go ./
COPY proto/tunnel.proto ./proto/

RUN go install google.golang.org/protobuf/cmd/protoc-gen-go@v1.33.0 \
    && go install google.golang.org/grpc/cmd/protoc-gen-go-grpc@v1.3.0
ENV PATH="/root/go/bin:${PATH}"
RUN protoc \
      --go_out=. \
      --go-grpc_out=. \
      --go_opt=paths=source_relative \
      --go-grpc_opt=paths=source_relative \
      proto/tunnel.proto

RUN go mod tidy && \
    CGO_ENABLED=0 GOOS=linux go build -o /out/tunnel-client .

FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=build /out/tunnel-client /tunnel-client
USER nonroot:nonroot
ENTRYPOINT ["/tunnel-client"]
