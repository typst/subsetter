fonttools subset fonts/NotoSans-Regular.ttf --drop-tables=GSUB,GPOS,GDEF \
 --gids=80-100,3,4,10,30,31,300-330 --glyph-names --output-file=out_ft.ttf \
 --notdef-outline --no-prune-unicode-ranges --name-IDs='*'
fonttools ttx -f -o out_ft.ttx out_ft.ttf