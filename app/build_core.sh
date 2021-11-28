#!/bin/bash

set -euxo pipefail

# We always expect the toolchain is already set up for all archs

__build() {
    cd "${SRCROOT}/../core"
    cargo build --release --target="$1"
}

arch_to_target() {
    case "$1" in
        "x86_64")
            echo "x86_64-apple-darwin"
            ;;
        "arm64")
            echo "aarch64-apple-darwin"
            ;;
        *)
            echo "Don't know how to build arch $1"
            exit 1
            ;;
    esac
}

build() {
    if [[ "${ONLY_ACTIVE_ARCH}" == "YES" ]]; then
        active_archs="${NATIVE_ARCH}"
    else
        active_archs=(${ARCHS})
    fi

    for arch in "${active_archs[@]}"; do
        __build "$(arch_to_target "${arch}")"
    done

    libs=()
    for arch in "${active_archs[@]}"; do
        libs+=( "${SRCROOT}/../core/target/$(arch_to_target "$arch")/release/libspecht2_core.a" )
    done

    lipo -create -output "${SRCROOT}/Specht2/core.a" "${libs[@]}"
}

build
