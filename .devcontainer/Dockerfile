# Use buster on Intel for testing Homebrew on Linux.
# https://hub.docker.com/_/microsoft-devcontainers-rust
# Please note that Homebrew on Linux does not support ARM processors.
FROM --platform=linux/x86_64 mcr.microsoft.com/devcontainers/rust:1.0.9-buster

# Clang 15
RUN apt-get update \
  && apt-get install -y \
    build-essential \
    autoconf \
    libtool \
    git \
    wget \
    software-properties-common \
  && wget -O - https://apt.llvm.org/llvm-snapshot.gpg.key|apt-key add - \
  && apt-add-repository "deb http://apt.llvm.org/buster/ llvm-toolchain-buster-15 main" \
  && apt-get update \
  && apt-get install -y clang-15 lldb-15 lld-15 \
  && apt-get clean \
  && rm -rf /var/lib/apt/lists/*

RUN ln -s /usr/bin/clang-15 /usr/bin/clang \
  && ln -s /usr/bin/clang++-15 /usr/bin/clang++

# Homebrew for Linux
USER vscode
RUN /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
ENV PATH=${PATH}:/home/linuxbrew/.linuxbrew/bin:/home/linuxbrew/.linuxbrew/sbin/
