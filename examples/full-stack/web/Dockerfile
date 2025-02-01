FROM denoland/deno:2.1.9 AS builder


RUN deno install -A --global jsr:@diskuto/web@0.1.7 --name diskuto-web

WORKDIR /config
COPY *.toml .

# Run with: diskuto-web start