fonttools subset fonts/NotoSansCJKsc-Regular.otf --drop-tables=GSUB,GPOS,GDEF,FFTM,vhea,vmtx,DSIG,VORG \
 --gids=416 --glyph-names --output-file=out_ft.otf \
 --notdef-outline --no-prune-unicode-ranges &&
fonttools ttx -f -o out_ft.ttx out_ft.otf