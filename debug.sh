GIDS="0"

cargo run -- fonts/NotoSans-Regular.ttf debug_ns.otf $GIDS > out_ns.txt
cargo run -- ss.otf debug_ss.otf $GIDS > out_ss.txt
cargo run -- ft.otf debug_ft.otf $GIDS > out_ft.txt

rm debug_ft.otf
rm debug_ss.otf
rm debug_ns.otf