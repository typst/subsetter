fonttools subset fonts/Roboto-Regular.ttf --drop-tables=GSUB,GPOS,GDEF,FFTM \
 --gids=* --glyph-names --output-file=out_ft.ttf \
 --notdef-outline --no-prune-unicode-ranges &&
fonttools ttx -f -o out_ft.ttx out_ft.ttf