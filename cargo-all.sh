#!/bin/bash

set -e

script_dir="$(cd $(dirname $0) && pwd)"
projects=(shared kernel-lib kernel bootloader)

for project in ${projects[@]}; do
  # currently tests are not executable on kernel and bootloader
  if [[ "$1" == "test" && ("$project" == "kernel" || "$project" == "bootloader") ]]; then
    continue
  fi
  cd "$script_dir/$project"
  cargo $@
done
