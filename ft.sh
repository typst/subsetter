fonttools subset fonts/NotoSans-Regular.ttf --drop-tables=GSUB,GPOS,GDEF \
 --gids=3,6,70,71,125,80-100 --glyph-names --output-file=out_ft.ttf \
 --notdef-outline --no-prune-unicode-ranges --name-IDs='*'
fonttools ttx -f -o out_ft.ttx out_ft.ttf