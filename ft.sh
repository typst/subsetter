fonttools subset fonts/LatinModernRoman-Regular.otf --drop-tables=GSUB,GPOS,GDEF,FFTM,vhea,vmtx,DSIG,VORG \
 --gids=400-420 --glyph-names --output-file=out_ft.otf \
 --notdef-outline --desubroutinize --no-prune-unicode-ranges &&
fonttools ttx -f -o out_ft.ttx out_ft.otf