# syntax=docker/dockerfile:1.7

# Stage 1: build the agentic binary.
FROM rust:1-slim-bookworm AS builder

# libgit2-sys (vendored via git2 dep) needs a C toolchain + zlib headers.
# pkg-config lets cargo discover the system zlib; everything else is
# pure Rust.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        pkg-config \
        zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY . .

RUN cargo install --path crates/agentic-cli --locked --root /out

# Stage 2: runtime image.
#
# ca-certificates kept for any future TLS use (~1 MB).
#
# No system `git` is installed: the CLI talks to repos via the
# vendored libgit2 linked into the binary (git2 crate), and no code
# path shells out to a `git` process. Adding the apt package would
# roughly double the image size.
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /out/bin/agentic /usr/local/bin/agentic

# libgit2's safe.directory check otherwise rejects a /workspace
# bind-mounted from the host (UID mismatch between host user and
# container root). Whitelist all directories — the container is
# ephemeral and intentionally isolated.
RUN printf '[safe]\n\tdirectory = *\n' > /etc/gitconfig

# Host bind-mount target: `docker run -v "$PWD":/workspace` lets the
# CLI see stories/ and the repo's git state.
WORKDIR /workspace

# Named volume target: persists the SurrealStore across container runs
# so UAT signings survive.
VOLUME ["/root/.local/share/agentic"]

ENTRYPOINT ["agentic"]
