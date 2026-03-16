default:
    @just --list

fmt:
    cargo fmt --all

lint:
    cargo xlint
    cd frontend && npm run lint

typecheck:
    cargo xtypecheck
    cd frontend && npm run typecheck

test:
    cargo xtest
    cd frontend && npm run test

build:
    cargo build --workspace
    cd frontend && npm run build
