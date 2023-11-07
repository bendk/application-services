#!/bin/bash

set -e

if [ -z "$1" ]
  then
    echo "Usage try_to_crash_places.sh [swift_script]"
    exit
fi

MEGAZORD_FILENAME="libmegazord.so"
ROOT_DIR=$(dirname "$(realpath $0)")
PLACES_DIR="$ROOT_DIR/components/places"
TEMP_DIR="$ROOT_DIR/target/try-to-crash-temp"
SCRIPT=$1
BINARY=$(basename $1 .swift) # Maybe this needs to be adjusted

cd $ROOT_DIR
cargo build -p megazord

mkdir -p $TEMP_DIR
cp $SCRIPT $TEMP_DIR
cp "$ROOT_DIR/target/debug/$MEGAZORD_FILENAME" $TEMP_DIR
cargo run -p embedded-uniffi-bindgen generate --library target/debug/libmegazord.so --out-dir $TEMP_DIR --language swift --no-format

cd $TEMP_DIR
swiftc -emit-module -module-name places_mod -emit-library -Xcc -fmodule-map-file=$(realpath placesFFI.modulemap) places.swift
swiftc -Xcc -fmodule-map-file=$(realpath placesFFI.modulemap) -L . -I . -l megazord -l places_mod $SCRIPT
for i in {0..100000}
do
    LD_LIBRARY_PATH=. ./$BINARY $(realpath places.db) $i
done
