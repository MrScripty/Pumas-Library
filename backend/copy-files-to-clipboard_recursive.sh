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
declare -a file_list

# Supported extensions
extensions=(
    py md rs ts js css html htm
    json yaml yml toml
    sh bash zsh ksh
    txt ini conf cfg
    xml svg sql c cpp java go
)

# Build the find command arguments for extensions
# This creates a search pattern like: -name "*.py" -o -name "*.md" ...
find_args=()
for ext in "${extensions[@]}"; do
    if [[ ${#find_args[@]} -gt 0 ]]; then
        find_args+=("-o")
    fi
    find_args+=("-name" "*.$ext")
done

# Find files recursively
# 1. Filters by extensions
# 2. Skips hidden directories (like .git or .vscode)
# 3. Excludes the script itself
# 4. Sorts the results alphabetically
while IFS= read -r file; do
    # Skip the script itself if it matches the extension list
    [[ "$(realpath "$file")" == "$(realpath "$0")" ]] && continue
    file_list+=("$file")
done < <(find "$SCRIPT_DIR" -type f \( "${find_args[@]}" \) -not -path '*/.*' | sort -f)

# Process each file
for file in "${file_list[@]}"; do
    # Get path relative to the script directory for cleaner labeling
    rel_path="${file#$SCRIPT_DIR/}"
    file_count=$((file_count + 1))

    # Determine language for syntax highlighting based on extension
    ext="${file##*.}"
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

    # Write formatted block
    printf '--- File: %s ---\n' "$rel_path" >> "$TEMP_FILE"
    printf '```%s\n' "$lang" >> "$TEMP_FILE"
    cat "$file" >> "$TEMP_FILE"
    printf '\n```\n\n' >> "$TEMP_FILE"
done

# Final feedback and copy
if [ $file_count -eq 0 ]; then
    echo "No matching files found in $SCRIPT_DIR (recursive search)."
else
    xclip -selection clipboard < "$TEMP_FILE"
    echo "Success! Processed $file_count file(s) recursively and copied to clipboard."
fi
