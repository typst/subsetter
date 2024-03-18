FONT="fonts/DejaVuSansMono.ttf"
GIDS="450-500"

fonttools subset $FONT --drop-tables=GSUB,GPOS,GDEF,FFTM,vhea,vmtx,DSIG,VORG,cmap,hdmx \
 --gids=$GIDS --glyph-names --canonical-order --output-file=out_ft.otf \
 --notdef-outline --no-prune-unicode-ranges &&
fonttools ttx -f -o out_ft.ttx out_ft.otf &&
cp out_ft.otf ft.otf

cargo run -- $FONT out_ss.otf $GIDS &&
fonttools ttx -f -o /Users/lstampfl/Programming/GitHub/subsetter/out_ss.ttx /Users/lstampfl/Programming/GitHub/subsetter/out_ss.otf
fonttools ttx -f -o /Users/lstampfl/Programming/GitHub/subsetter/out_ss.ttx /Users/lstampfl/Programming/GitHub/subsetter/out_ss.otf &&
cp out_ss.otf ss.otf