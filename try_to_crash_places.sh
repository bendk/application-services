#!/bin/bash

set -e

MEGAZORD_FILENAME="libmegazord.so"
ROOT_DIR=$(dirname "$(realpath $0)")
PLACES_DIR="$ROOT_DIR/components/places"
TEMP_DIR="$ROOT_DIR/target/try-to-crash-temp"

cd $ROOT_DIR
cargo build -p megazord

cp try_to_crash_places.swift $TEMP_DIR
cp "$ROOT_DIR/target/debug/$MEGAZORD_FILENAME" $TEMP_DIR
cargo run -p embedded-uniffi-bindgen generate --library target/debug/libmegazord.so --out-dir $TEMP_DIR --language swift --no-format

cd $TEMP_DIR
swiftc -emit-module -module-name places_mod -emit-library -Xcc -fmodule-map-file=$(realpath placesFFI.modulemap) places.swift
swiftc -Xcc -fmodule-map-file=$(realpath placesFFI.modulemap) -L . -I . -l megazord -l places_mod try_to_crash_places.swift
for i in {0..100000}
do
    echo $i
    LD_LIBRARY_PATH=. ./try_to_crash_places $(realpath places.db)
done
