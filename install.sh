#!/bin/bash

INSTALLATION_PATH=${1}

echo "Installing modelflat_bot..."

cargo clean && cargo build --release

mkdir "${INSTALLATION_PATH}"
cp $(find target -type f -executable -name modelflat_bot -print) ${INSTALLATION_PATH}
