#!/usr/bin/env bash
set -e

# Counts number of files/lines for each entry of the "Repository Overview" using `cloc`

cloc ./benchmarks/{plots,scripts,Makefile} ./src ./Cargo.toml --json | jq -c '{ files: .SUM.nFiles, lines: [.SUM.code, .SUM.comment] | add }'

printf './benchmarks: '
cloc ./benchmarks/{plots,scripts,Makefile} --json | jq -c '{ files: .SUM.nFiles, lines: [.SUM.code, .SUM.comment] | add }'

ENTRIES=(
  "./benchmarks/plots"
  "./benchmarks/scripts"
  "./benchmarks/Makefile"
  "./src"
  "./src/class"
  "./src/class/descriptors"
  "./src/function"
  "./src/function/structure"
  "./src/function/locals.rs"
  "./src/function/visitor.rs"
  "./src/graph"
  "./src/output"
  "./src/output/builtin"
  "./src/tests"
  "./src/virtuals"
  "./src/main.rs"
  "./src/options.rs"
  "./src/scheduler.rs"
  "./Cargo.toml"
)
for ENTRY in "${ENTRIES[@]}" ; do
    printf "%s: " "$ENTRY"
    cloc "$ENTRY" --json | jq -c '{ files: .SUM.nFiles, lines: [.SUM.code, .SUM.comment] | add }'
done
