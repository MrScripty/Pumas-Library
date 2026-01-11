#!/bin/bash

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Check if xclip is installed
if ! command -v xclip &> /dev/null; then
    echo "Error: xclip is not installed. Please install it (e.g., 'sudo apt install xclip')."
    exit 1
fi

# Temporary file for final output
TEMP_FILE=$(mktemp)
trap 'rm -f "$TEMP_FILE"' EXIT

file_count=0
declare -a file_list  # To store full paths for sorting

shopt -s nullglob

# Supported extensions (add more if needed)
extensions=(
    py md rs ts js css html htm
    json yaml yml toml
    sh bash zsh ksh
    txt ini conf cfg
    xml svg sql c cpp java go rs
)

# Collect all matching files
for ext in "${extensions[@]}"; do
    for file in "$SCRIPT_DIR"/*."$ext"; do
        [[ -f "$file" ]] || continue
        # Skip the script itself
        [[ "$file" != "$0" ]] || continue
        file_list+=("$file")
    done
done

# Sort files alphabetically by basename
IFS=$'\n' sorted_files=($(printf '%s\n' "${file_list[@]}" | sort -f))

# Process each file in sorted order
for file in "${sorted_files[@]}"; do
    filename=$(basename "$file")
    file_count=$((file_count + 1))

    # Determine language for syntax highlighting
    ext="${filename##*.}"
    ext="${ext,,}"

    case "$ext" in
        py)           lang="python" ;;
        md)           lang="markdown" ;;
        rs)           lang="rust" ;;
        ts)           lang="typescript" ;;
        js)           lang="javascript" ;;
        css)          lang="css" ;;
        html|htm)     lang="html" ;;
        json)         lang="json" ;;
        yaml|yml)     lang="yaml" ;;
        toml)         lang="toml" ;;
        sh|bash|zsh|ksh) lang="bash" ;;
        txt)          lang="text" ;;
        ini|conf|cfg) lang="ini" ;;
        xml)          lang="xml" ;;
        svg)          lang="svg" ;;
        sql)          lang="sql" ;;
        c)            lang="c" ;;
        cpp)          lang="cpp" ;;
        java)         lang="java" ;;
        go)           lang="go" ;;
        *)            lang="" ;;
    esac

    # Write formatted block exactly as you want
    printf '%s\n\n' "$filename" >> "$TEMP_FILE"
    printf '`````%s\n' "$lang" >> "$TEMP_FILE"
    cat "$file" >> "$TEMP_FILE"
    printf '\n`````\n\n' >> "$TEMP_FILE"
done

# Final feedback and copy
if [ $file_count -eq 0 ]; then
    printf 'No supported text files found in:\n%s\n' "$SCRIPT_DIR" > "$TEMP_FILE"
    echo "No files found in $SCRIPT_DIR"
else
    echo "Success! Processed and copied $file_count file(s) in alphabetical order."
fi

# Copy to clipboard
xclip -selection clipboard < "$TEMP_FILE"
