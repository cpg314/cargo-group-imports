#!/usr/bin/env sh
set -euo pipefail

VERSION=$(tomlq -r ".package.version" Cargo.toml)
PACKAGE=$(tomlq -r ".package.name" Cargo.toml)
for ARCH in x86_64-unknown-linux-gnu 
do
    cross build -r --target $ARCH --target-dir target-cross
    OUT=target-cross/$ARCH/release/
    cargo about generate about.hbs > $OUT/licenses.html
    DEST=target/$PACKAGE-$VERSION-$ARCH.tar.gz
    echo $DEST
    tar czf $DEST -C $OUT $PACKAGE licenses.html
    tar tvf $DEST
done
