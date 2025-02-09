test:
  cargo test

install-docs:
  cargo install mdbook
  cargo install mdbook-keeper@0.3.0

setup-docs:
  cargo install mdbook-keeper

serve-docs:
  (cd docs && mdbook serve --open)

build-docs:
  (cd docs && mdbook build)

doc-test:
  cargo test --doc
  mdbook build docs

format:
  cargo fmt --all
  find . -type f -iname "*.toml" -print0 | xargs -0 taplo format


