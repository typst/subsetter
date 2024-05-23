FONT="/Users/lstampfl/Desktop/fonts-main/ofl/handjet/Handjet[ELGR,ELSH,wght].ttf"
GIDS="419"

# Run normally

fonttools subset $FONT --drop-tables=GSUB,GPOS,GDEF,FFTM,vhea,vmtx,DSIG,VORG,hdmx,cmap,HVAR,MVAR,STAT,avar,fvar,gvar \
 --gids=$GIDS --glyph-names --desubroutinize --output-file=out_ft.otf \
 --notdef-outline --no-prune-unicode-ranges --no-prune-codepage-ranges &&
fonttools ttx -f -o out_ft.xml out_ft.otf

cargo run -- $FONT out_ss.otf $GIDS &&
fonttools ttx -f -o out_ss.xml out_ss.otf

#hb-subset $FONT --desubroutinize --gids=$GIDS --output-file=out_hb.otf &&
#fonttools ttx -f -o out_hb.xml out_hb.otf

# Bench against hb-subset
#time ./target/release/subsetter $FONT out_ss.otf $GIDS
#time hb-subset $FONT --desubroutinize --gids=$GIDS --output-file=out_hb.otf
