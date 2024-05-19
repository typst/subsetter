FONT="fonts/NotoSansCJKsc-Regular.otf"
GIDS="0,1,2698,3950,4059,8509,14538,38945"

fonttools subset $FONT --drop-tables=GSUB,GPOS,GDEF,FFTM,vhea,vmtx,DSIG,VORG,hdmx,cmap \
 --gids=$GIDS --glyph-names --desubroutinize --output-file=out_ft.otf \
 --notdef-outline --no-prune-unicode-ranges --no-prune-codepage-ranges &&
fonttools ttx -f -o out_ft.xml out_ft.otf &&

cargo run -- $FONT out_ss.otf $GIDS &&
fonttools ttx -f -o out_ss.xml out_ss.otf


#time hb-subset $FONT --desubroutinize --gids=$GIDS --output-file=out_hb.otf

#cargo run -- $FONT out_ss.otf $GIDS &&
#fonttools ttx -f -o /Users/lstampfl/Programming/GitHub/subsetter/out_ss.ttx /Users/lstampfl/Programming/GitHub/subsetter/out_ss.otf &&
#cp out_ss.otf ss.otf
#
#
#fonttools subset fonts/NotoSans-Regular.ttf --drop-tables=GSUB,GPOS,GDEF,FFTM,vhea,vmtx,DSIG,VORG,cmap,hdmx \
# --gids=5-20 --glyph-names --output-file=out_ft.otf \
# --notdef-outline --no-prune-unicode-ranges --no-prune-codepage-ranges --timing