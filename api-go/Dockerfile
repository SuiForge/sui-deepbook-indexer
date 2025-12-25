FROM golang:1.24-alpine AS builder
WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 go build -o /api ./cmd

FROM alpine:latest
RUN apk --no-cache add ca-certificates
COPY --from=builder /api /api
EXPOSE 8080
CMD ["/api"]
