FROM golang:1.26.3 AS builder
WORKDIR /build
COPY ./ ./
RUN go mod download
RUN CGO_ENABLED=0 go build -o ./main


FROM scratch
WORKDIR /app
COPY --from=builder /build/main ./main
EXPOSE 8080
ENTRYPOINT ["./main"]
