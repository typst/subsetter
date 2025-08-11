use std::env;
use subsetter::{subset, GlyphRemapper};

fn parse_gids(gids: &str) -> Vec<u16> {
    if gids == "*" {
        return (0..u16::MAX).collect();
    }

    let split = gids.split(',').filter(|s| !s.is_empty()).collect::<Vec<_>>();
    let mut gids = vec![];

    for el in &split {
        if el.contains('-') {
            let range = el.split('-').collect::<Vec<_>>();
            let first = range[0].parse::<u16>().unwrap();
            let second = range[1].parse::<u16>().unwrap();

            gids.extend(first..=second);
        } else {
            gids.push(el.parse::<u16>().unwrap());
        }
    }

    gids
}

// Note that this is more of an experimental CLI used for testing.
fn main() {
    let args: Vec<String> = env::args().collect();
    let data = std::fs::read(&args[1]).unwrap();
    let gids = parse_gids(args.get(3).to_owned().unwrap_or(&"0-5".to_owned()));
    let remapper = GlyphRemapper::new_from_glyphs(gids.as_slice());

    let sub = subset(&data, 0, &[], &remapper).unwrap();

    std::fs::write(args.get(2).unwrap_or(&"res.otf".to_owned()), sub).unwrap();
}
