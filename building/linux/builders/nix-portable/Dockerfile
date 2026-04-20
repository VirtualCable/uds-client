FROM debian:stable

# Install basic dependencies to run nix-portable
RUN apt-get update && apt-get install -y \
  curl git xz-utils bzip2 ca-certificates libfuse3-3 \
  && rm -rf /var/lib/apt/lists/*

# Download nix-portable
RUN curl -L https://github.com/DavHau/nix-portable/releases/latest/download/nix-portable-$(uname -m) -o /usr/local/bin/nix-portable \
    && chmod +x /usr/local/bin/nix-portable

# Initialize nix-portable to avoid first-run delays
RUN NP_RUNTIME=bwrap nix-portable nix --version

# Copy the build script
COPY build-nix.sh /usr/local/bin/build-nix.sh
RUN chmod +x /usr/local/bin/build-nix.sh

