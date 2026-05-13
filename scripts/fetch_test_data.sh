#!/bin/bash
set -e

DATA_DIR="tests/parity_data"
mkdir -p "$DATA_DIR"

download_templates() {
    local repo_url=$1
    local name=$2
    local temp_dir=$(mktemp -d)
    
    echo "Fetching $name templates..."
    git clone --depth 1 "$repo_url" "$temp_dir" --quiet
    
    mkdir -p "$DATA_DIR/$name"
    # Find all directories named 'templates' and copy their contents
    find "$temp_dir" -type d -name "templates" -exec cp -r {}/. "$DATA_DIR/$name/" \;
    
    rm -rf "$temp_dir"
}

download_templates "https://github.com/thraxil/sebastian" "sebastian"
download_templates "https://github.com/thraxil/gearspotting" "gearspotting"

echo "Templates downloaded to $DATA_DIR"
