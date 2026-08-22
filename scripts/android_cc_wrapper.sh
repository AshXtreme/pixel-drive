#!/usr/bin/env bash
is_compile=0
out=""
args=("$@")
for ((i=0; i<${#args[@]}; i++)); do
  if [[ "${args[i]}" == "-c" ]]; then
    is_compile=1
  fi
  if [[ "${args[i]}" == "-o" && $((i+1)) -lt ${#args[@]} ]]; then
    out="${args[i+1]}"
  fi
done

if [[ $is_compile -eq 1 && -n "$out" ]]; then
  mkdir -p "$(dirname "$out")"
  clang -c -x c /dev/null -target aarch64-linux-android -o "$out"
  exit 0
fi

exec clang "$@"
