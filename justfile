default: check

check:
    cargo fmt --all -- --check
    cargo clippy --all --all-targets --all-features -- -D warnings
    cargo test --all-features

fmt:
    cargo fmt --all

install-hooks:
    cp .githooks/pre-push .git/hooks/pre-push
