use std::env;
use subsetter::{subset, GidMapper};

fn parse_gids(gids: &str) -> Vec<u16> {
    if gids == "*" {
        return (0..u16::MAX).collect();
    }

    let split = gids.split(",").filter(|s| !s.is_empty()).collect::<Vec<_>>();
    let mut gids = vec![];

    for el in &split {
        if el.contains("-") {
            let range = el.split("-").collect::<Vec<_>>();
            let first = range[0].parse::<u16>().unwrap();
            let second = range[1].parse::<u16>().unwrap();

            gids.extend(first..=second);
        } else {
            gids.push(el.parse::<u16>().unwrap());
        }
    }

    gids
}

fn main() {
    let args: Vec<String> = env::args().collect();
    // Read the raw font data.
    let data = std::fs::read(&args[1]).unwrap();
    let gids = parse_gids(&args.get(3).to_owned().unwrap_or(&"0-20".to_owned()));
    let mapper = GidMapper::from_gid_set(&gids);

    let sub = subset(&data, 0, &mapper).unwrap();

    // Write the resulting file.
    std::fs::write(&args.get(2).unwrap_or(&"res.otf".to_owned()), sub).unwrap();
}
