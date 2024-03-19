FROM mcr.microsoft.com/devcontainers/rust:1-1-bullseye

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
  && apt-add-repository "deb http://apt.llvm.org/bullseye/ llvm-toolchain-bullseye-15 main" \
  && apt-get update \
  && apt-get install -y clang-15 lldb-15 lld-15 \
  && apt-get clean \
  && rm -rf /var/lib/apt/lists/*

RUN ln -s /usr/bin/clang-15 /usr/bin/clang \
  && ln -s /usr/bin/clang++-15 /usr/bin/clang++
