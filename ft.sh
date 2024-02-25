fonttools subset fonts/MPLUS1p-Regular.ttf --drop-tables=GSUB,GPOS,GDEF,FFTM,vhea,vmtx \
 --gids=0,1 --glyph-names --output-file=out_ft.ttf \
 --notdef-outline --no-prune-unicode-ranges &&
fonttools ttx -f -o out_ft.ttx out_ft.ttf