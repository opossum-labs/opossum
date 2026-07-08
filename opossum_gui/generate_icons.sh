#!/bin/bash

# Target directory for the generated icons
OUTPUT_DIR="assets/icons"
mkdir -p "$OUTPUT_DIR"

# Standard sizes required for full Linux desktop compatibility
SIZES=(32 64 128 256 512)

# Loop through all sizes and export from the SVG source
for SIZE in "${SIZES[@]}"; do
    echo "Rendering ${SIZE}x${SIZE}.png from SVG..."
    inkscape --export-filename="${OUTPUT_DIR}/${SIZE}x${SIZE}.png" -w "$SIZE" -h "$SIZE" /home/udo/dev/opossum/opossum_core/logo/Logo_square.svg
done

echo "Icon generation complete!"
