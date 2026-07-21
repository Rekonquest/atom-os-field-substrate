# ATOM OS Field Substrate Development Container
# Based on Rust official image with additional tools

FROM ghcr.io/rust-lang/rust:nightly-slim

# Install system dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    git \
    curl \
    build-essential \
    libm-dev \
    cmake \
    pkg-config \
    qemu-system-x86 \
    nasm \
    && rm -rf /var/lib/apt/lists/*

# Install GitHub CLI
RUN curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | gpg --dearmor -o /usr/share/keyrings/githubcli-archive-keyring.gpg && \
    echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | tee /etc/apt/sources.list.d/github-cli.list > /dev/null && \
    apt-get update && \
    apt-get install -y gh && \
    rm -rf /var/lib/apt/lists/*

# Install bootimage for kernel development (optional, for full kernel builds)
RUN cargo install bootimage --locked

# Set up workspace
WORKDIR /workspace

# Configure git
RUN git config --global --add safe.directory /workspace

# Environment variables
ENV RUST_BACKTRACE=1
ENV CARGO_TERM_COLOR=true

# Default command
CMD ["sleep", "infinity"]
