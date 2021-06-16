#!/bin/bash -e

# Installs git hooks for cargo commands

case "$1" in
    install)
        echo './git-hooks.sh pre-commit' > .git/hooks/pre-commit
        echo './git-hooks.sh pre-push' > .git/hooks/pre-push

        chmod +x .git/hooks/pre-commit
        chmod +x .git/hooks/pre-push

        ;;

    pre-commit)
        echo "Running format check"
        cargo fmt -- --check
        echo "Running clippy"
        cargo clippy --all

        ;;

    pre-push)
        $0 pre-commit
        echo "Running tests"
        cargo test

        ;;
    *)
        echo "Use $0 install to install git hooks"
        exit 1
esac
