#!/bin/bash

set -e

script_dir="$(cd $(dirname $0) && pwd)"
projects=(shared kernel-lib kernel bootloader)

for project in ${projects[@]}; do
  cd "$script_dir/$project"
  cargo $@
done
