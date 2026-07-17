#!/usr/bin/env bash

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

for file in $DIR/*.yaml; do
  echo "Generating $file"
  redocly build-docs $file -o "${file}.html"
done
