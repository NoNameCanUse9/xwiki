# ---- Frontend build ----
FROM node:26-alpine AS web
WORKDIR /app
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# ---- Go build ----
FROM golang:1.26-alpine AS build
WORKDIR /src
COPY go.mod go.sum ./
RUN go mod download
COPY . .
COPY --from=web /app/dist ./web/dist
RUN CGO_ENABLED=0 go build -o /out/xwiki ./cmd/xwiki

# ---- Runtime ----
FROM alpine:3.22
RUN apk add --no-cache git ca-certificates
COPY --from=build /out/xwiki /usr/local/bin/xwiki
ENV XWIKI_DATA_DIR=/data
EXPOSE 8080
ENTRYPOINT ["xwiki"]
CMD ["serve"]
