# The three real language servers, in one image — `CP-4`'s biggest mechanical
# gap closed.
#
# `CP-4` asks for *"completion + signature help against rust-analyzer, tsserver,
# pyright"*, and until this image exactly one of the three had ever been
# attached by anything automated. `crates/phosphor-buffer/tests/lsp_servers.rs`
# is the test; this is the only place all three exist at once.
#
# ## Why a container rather than "install them and run the tests"
#
# Because the alternative is what the repository already had: a test that skips
# when the tool is missing, on a machine where the tool is usually missing. That
# skip is deliberate and stays — a test that reddens CI for a missing binary
# trains everyone to ignore a red build — but a skip that *always* fires is
# indistinguishable from no test. This image is where it does not fire.
#
# It is also the honest answer to a `T036` finding: `rust-analyzer` on the
# machine that task was written on was a rustup shim that exited 1, so
# `which` found a server that could not serve. A container pins what is
# actually there.
#
# ## Node gives two of the three
#
# `pyright-langserver` ships in the **npm** `pyright` package, not in a Python
# distribution — so Node covers both it and `typescript-language-server`, and no
# Python interpreter is installed at all. That is most of why this image is as
# small as it is.
#
# ## Versions are pinned, and the reason is `rust-toolchain.toml`'s
#
# *"A floor doesn't give you that — two machines both satisfying the floor can
# still differ."* A language server's completion list is exactly the kind of
# output that moves between versions, so a test asserting on one wants to know
# which. Bumping a pin here is a deliberate, reviewed commit.
#
# Build and run it with `just lsp-docker`; the recipe is the interface and this
# file is an implementation detail of it.
#
# ## What it found on its first run
#
# Four things, all recorded at `T036` in `docs/TASKS.md` — which is the argument
# for the image better than anything above it: two servers that had never been
# attached had four true things to say the moment anything asked them.

# Matches `rust-toolchain.toml`. `slim` rather than the default: the full image
# carries a second toolchain's worth of documentation and sources that nothing
# here reads.
FROM rust:1.97.1-slim-bookworm

# `ca-certificates` for the npm registry, `git` because cargo fetches over it,
# `curl` and `gnupg` to add nodesource. `--no-install-recommends` keeps this
# from pulling a desktop's worth of suggestions.
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        git \
        gnupg \
    && rm -rf /var/lib/apt/lists/*

# Node 22 (LTS). The two servers below are npm packages and this is the only
# reason node is here — nothing in phosphor builds with it.
ARG NODE_MAJOR=22
RUN curl -fsSL "https://deb.nodesource.com/setup_${NODE_MAJOR}.x" | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

# The two npm-distributed servers, pinned.
#
# `typescript-language-server` is the tsserver *wrapper*, which is what
# `runtime/languages/typescript.scm` declares and why — tsserver itself speaks
# its own protocol, not LSP. `typescript` is its peer dependency and is what it
# actually drives.
ARG TS_SERVER_VERSION=5.3.0
ARG TYPESCRIPT_VERSION=5.7.3
ARG PYRIGHT_VERSION=1.1.391
RUN npm install --global --no-fund --no-audit \
        "typescript-language-server@${TS_SERVER_VERSION}" \
        "typescript@${TYPESCRIPT_VERSION}" \
        "pyright@${PYRIGHT_VERSION}" \
    && npm cache clean --force

# rust-analyzer as a rustup component, so it is the one built for this exact
# toolchain rather than whatever a release tarball happens to be.
RUN rustup component add rust-analyzer

# `cargo-nextest`, because `just test` is nextest and a container that ran
# `cargo test` would be running a different thing from the rest of the build.
# Installed from the pre-built binary rather than compiled: this is a test
# runner, not a dependency, and building it costs several minutes per image.
#
# **`.../latest/linux` is amd64 only**, and on an arm64 host that produces an
# image which builds clean and then dies at run time with
#
#     rosetta error: failed to open elf at /lib64/ld-linux-x86-64.so.2
#
# from a binary nothing in the Dockerfile appears to have chosen. `TARGETARCH`
# is buildx's own answer for the platform being built, so the URL is derived
# rather than assumed.
ARG TARGETARCH
RUN set -eux; \
    case "${TARGETARCH}" in \
        arm64) nextest_url=https://get.nexte.st/latest/linux-arm ;; \
        amd64) nextest_url=https://get.nexte.st/latest/linux ;; \
        *) echo "no cargo-nextest build for ${TARGETARCH}" >&2; exit 1 ;; \
    esac; \
    curl -fsSL "$nextest_url" | tar zxf - -C /usr/local/cargo/bin; \
    cargo nextest --version

# **The probe every one of these has to pass**, run at build time so a broken
# image fails here rather than as a mysteriously skipped test. This is
# `lsp_servers.rs`'s own `usable()` check, one layer out.
#
# **Two exit codes, and the second one is not a failure.** `--version` exiting 0
# is what separates a server from a shim that merely exists on `PATH` — but
# `pyright-langserver --version` exits **1**, objecting that no transport was
# named:
#
#     Error: Connection input stream is not set. Use arguments of
#     createConnection or set command line parameters: '--node-ipc',
#     '--stdio' or '--socket={number}'
#
# That is the server parsing its arguments, which is proof it started. This
# build failed on it before the check was written to match what
# `lsp_servers.rs::usable` does, and the two agreeing is the point: an image
# that builds is an image where the test will not skip.
RUN set -eux; \
    rust-analyzer --version; \
    typescript-language-server --version; \
    pyright-langserver --version 2>&1 \
        | grep -q 'Connection input stream is not set'

WORKDIR /phosphor

# `CARGO_TARGET_DIR` off the bind mount: the host's `target/` holds macOS
# objects and sharing it would make every run a full rebuild in one direction
# or the other. A named volume mounted here by `just lsp-docker` is what makes
# the second run fast.
ENV CARGO_TARGET_DIR=/phosphor-target

# Nothing runs by default. `just lsp-docker` passes the command, so the image
# is equally usable for a shell when a server misbehaves and the question is
# what it actually said.
CMD ["cargo", "nextest", "run", "-p", "phosphor-buffer", "--test", "lsp_servers", "--no-capture"]
