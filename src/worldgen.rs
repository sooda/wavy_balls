use math::*;

#[derive(Debug, Copy, Clone)]
pub enum Shape {
    Flat(f32),
    Left(f32, f32),
    Right(f32, f32),
    Up(f32, f32),
    Down(f32, f32),
    LeftUp(f32, f32),
}

#[derive(Debug, Copy, Clone)]
pub struct Tile {
    shape: Shape
}

pub struct Worldgen {
    dims: (u32, u32),
    tiles: Vec<Tile>,
}

impl Worldgen {
    pub fn new(dims: (u32, u32)) -> Worldgen {
        Worldgen {
            dims: dims,
            tiles: vec![Tile { shape: Shape::Flat(0.0) }; (dims.0 * dims.1) as usize],
        }
    }
}

pub fn load_level_from_desc(desc: &str) -> Worldgen {
    use std::collections::HashMap;

    #[allow(dead_code)]
    fn parse_bool(v: &str) -> bool {
        match v {
            "true" | "yes" | "t" | "y" => true,
            "false" | "no" | "f" | "n" => false,
            x => panic!("invalid boolean value '{}'", x)
        }
    }

    let mut reading_tiles = false;
    let mut y = 0;
    let mut width = 0;
    let mut world = Worldgen::new((0,0));
    let mut blocks: HashMap<char, Tile> = HashMap::new();

    for line in desc.lines() {
        let line = line.trim();

        if line.is_empty() || (line.chars().nth(0).unwrap() == '#' && !reading_tiles) {
            continue
        }

        if reading_tiles {
            if y == 0 {
                width = line.len();
            } else {
                assert_eq!(line.len(), width);
            }

            for ch in line.chars() {
                let desc = blocks[&ch];
                world.tiles.push(desc);
            }

            y += 1;
        } else {
            let mut tokens = line.split_whitespace();
            let command = tokens.next().unwrap();
            let mut args = HashMap::new();
            for token in tokens {
                let mut kvs = token.splitn(2, '=');
                let key = kvs.next().unwrap();
                let value = kvs.next().unwrap();
                args.insert(key, value);
            }
            match command {
                "tiles" => {
                    reading_tiles = true;
                }
                "block" => {
                    let ch = args["ch"].chars().nth(0).unwrap();
//                     let tile = Tile {
//                         appearance: match *args.get("appearance").unwrap() {
//                             "stone" => TileAppearance::Stone,
//                             "plate" => TileAppearance::Plate,
//                             "plant" => TileAppearance::Plant,
//                             _ => panic!("invalid appearance for tile")
//                         },
//                         blocks_move: parse_bool(*args.get("block_move").unwrap_or(&"false")),
//                         blocks_light: parse_bool(*args.get("block_vis").unwrap_or(&"false")),
//                         height: args.get("height").unwrap_or(&"0.0").parse().unwrap()
//                     };
//                     let tile = Tile {
//                         height: args.get("height").unwrap_or(&"0.0").parse().unwrap()
//                     };
                    let tile = Tile {
                        shape: match *args.get("shape").unwrap_or(&"flat") {
                            "flat" => Shape::Flat(args.get("height").unwrap().parse().unwrap()),
                            "l" => Shape::Left(args.get("height").unwrap().parse().unwrap(),
                                               args.get("heightb").unwrap().parse().unwrap()),
                            "r" => Shape::Right(args.get("height").unwrap().parse().unwrap(),
                                                args.get("heightb").unwrap().parse().unwrap()),
                            "u" => Shape::Up(args.get("height").unwrap().parse().unwrap(),
                                                args.get("heightb").unwrap().parse().unwrap()),
                            "d" => Shape::Down(args.get("height").unwrap().parse().unwrap(),
                                                args.get("heightb").unwrap().parse().unwrap()),
                            "lu" => Shape::LeftUp(args.get("height").unwrap().parse().unwrap(),
                                                args.get("heightb").unwrap().parse().unwrap()),
                            s => panic!("invalid tile shape {}", s)
                        }
                    };
                    blocks.insert(ch, tile);
                }
//                 "physent" => {
//                     let pos_t = args["pos"];
//                     let mut xy_t = pos_t.split(',');
//                     let mut pos = Pnt2::new(xy_t.next().unwrap().parse().unwrap(),
//                                                 xy_t.next().unwrap().parse().unwrap());
//                     pos.x += 0.5;
//                     pos.y += 0.5;
//                     match args["type"] {
//                         "spawn" => world.spawns.push(pos),
//                         ty => panic!("unknown entity type '{}'", ty)
//                     }
//                 }
//                 "gfxent" => {
//                     let pos_t = args["pos"];
//                     let mut xyz_t = pos_t.split(',');
//                     let mut pos = Pnt3::new(xyz_t.next().unwrap().parse().unwrap(),
//                                                 xyz_t.next().unwrap().parse().unwrap(),
//                                                 xyz_t.next().unwrap().parse().unwrap());
//                     pos.x += 0.5;
//                     pos.z += 0.5;
//                     match args["type"] {
//                         "pointlight" => {
//                             let r = args.get("r").unwrap_or(&"1.0").parse().unwrap();
//                             let g = args.get("g").unwrap_or(&"1.0").parse().unwrap();
//                             let b = args.get("b").unwrap_or(&"1.0").parse().unwrap();
//                             world.lights.push(Light::Point(pos, Vec3::new(r,g,b)));
//                         }
//                         ty => panic!("unknown entity type '{}'", ty)
//                     }
//                 }
                t => {
                    panic!("Unknown instruction '{}' in level data", t);
                }
            }
        }
    }

    world.dims = (width as u32, y);

    world
}

pub fn generate_3d_vertices(world: &Worldgen) -> (Vec<Pnt3>, Vec<Vec3>, Vec<Pnt2>) {
    let mut pos = vec![];
    let mut nor = vec![];
    let mut tex = vec![];

    let sc = 10.0;

    for x in 0..world.dims.0 {
        for y in 0..world.dims.1 {
            let tile = &world.tiles[(y*world.dims.0+x) as usize];
            let vx = x as f32 * sc;
            let vz = y as f32 * sc;
            let (vy_0, vy_1, vy_2, vy_3) = match tile.shape {
                Shape::Left(d,u)  => (d,d,u,u),
                Shape::Right(d,u) => (u,u,d,d),
                Shape::Up(d,u)    => (d,u,d,u),
                Shape::Down(d,u)  => (u,d,u,d),
                Shape::LeftUp(d,u) => (d,d,d,u),
                Shape::Flat(h)    => (h,h,h,h),
            };

            pos.push(Pnt3::new(vx, vy_0, vz)); nor.push(Vec3::new(0.0, 1.0, 0.0)); tex.push(Pnt2::new(0.0, 0.0));
            pos.push(Pnt3::new(vx, vy_1, vz+sc)); nor.push(Vec3::new(0.0, 1.0, 0.0)); tex.push(Pnt2::new(0.0, 1.0));
            pos.push(Pnt3::new(vx+sc, vy_3, vz+sc)); nor.push(Vec3::new(0.0, 1.0, 0.0)); tex.push(Pnt2::new(0.5, 1.0));

            pos.push(Pnt3::new(vx+sc, vy_3, vz+sc)); nor.push(Vec3::new(0.0, 1.0, 0.0)); tex.push(Pnt2::new(0.5, 1.0));
            pos.push(Pnt3::new(vx+sc, vy_2, vz)); nor.push(Vec3::new(0.0, 1.0, 0.0)); tex.push(Pnt2::new(0.5, 0.0));
            pos.push(Pnt3::new(vx, vy_0, vz)); nor.push(Vec3::new(0.0, 1.0, 0.0)); tex.push(Pnt2::new(0.0, 0.0));

            pos.push(Pnt3::new(vx, vy_1, vz+sc)); nor.push(Vec3::new(-1.0, 0.0, 0.0)); tex.push(Pnt2::new(1.0, 1.0));
            pos.push(Pnt3::new(vx, 0.0, vz+sc)); nor.push(Vec3::new(-1.0, 0.0, 0.0)); tex.push(Pnt2::new(1.0, 0.0));
            pos.push(Pnt3::new(vx, 0.0, vz)); nor.push(Vec3::new(-1.0, 0.0, 0.0)); tex.push(Pnt2::new(0.5, 0.0));

            pos.push(Pnt3::new(vx, 0.0, vz)); nor.push(Vec3::new(-1.0, 0.0, 0.0)); tex.push(Pnt2::new(0.5, 0.0));
            pos.push(Pnt3::new(vx, vy_0, vz)); nor.push(Vec3::new(-1.0, 0.0, 0.0)); tex.push(Pnt2::new(0.5, 1.0));
            pos.push(Pnt3::new(vx, vy_1, vz+sc)); nor.push(Vec3::new(-1.0, 0.0, 0.0)); tex.push(Pnt2::new(1.0, 1.0));

            pos.push(Pnt3::new(vx+sc, vy_3, vz+sc)); nor.push(Vec3::new(1.0, 0.0, 0.0)); tex.push(Pnt2::new(0.5, 1.0));
            pos.push(Pnt3::new(vx+sc, 0.0, vz+sc)); nor.push(Vec3::new(1.0, 0.0, 0.0)); tex.push(Pnt2::new(0.5, 0.0));
            pos.push(Pnt3::new(vx+sc, 0.0, vz)); nor.push(Vec3::new(1.0, 0.0, 0.0)); tex.push(Pnt2::new(1.0, 0.0));

            pos.push(Pnt3::new(vx+sc, 0.0, vz)); nor.push(Vec3::new(1.0, 0.0, 0.0)); tex.push(Pnt2::new(1.0, 0.0));
            pos.push(Pnt3::new(vx+sc, vy_2, vz)); nor.push(Vec3::new(1.0, 0.0, 0.0)); tex.push(Pnt2::new(1.0, 1.0));
            pos.push(Pnt3::new(vx+sc, vy_3, vz+sc)); nor.push(Vec3::new(1.0, 0.0, 0.0)); tex.push(Pnt2::new(0.5, 1.0));

            pos.push(Pnt3::new(vx+sc, vy_2, vz)); nor.push(Vec3::new(0.0, 0.0, -1.0)); tex.push(Pnt2::new(0.5, 1.0));
            pos.push(Pnt3::new(vx+sc, 0.0, vz)); nor.push(Vec3::new(0.0, 0.0, -1.0)); tex.push(Pnt2::new(0.5, 0.0));
            pos.push(Pnt3::new(vx, 0.0, vz)); nor.push(Vec3::new(0.0, 0.0, -1.0)); tex.push(Pnt2::new(1.0, 0.0));

            pos.push(Pnt3::new(vx, 0.0, vz)); nor.push(Vec3::new(0.0, 0.0, -1.0)); tex.push(Pnt2::new(1.0, 0.0));
            pos.push(Pnt3::new(vx, vy_0, vz)); nor.push(Vec3::new(0.0, 0.0, -1.0)); tex.push(Pnt2::new(1.0, 1.0));
            pos.push(Pnt3::new(vx+sc, vy_2, vz)); nor.push(Vec3::new(0.0, 0.0, -1.0)); tex.push(Pnt2::new(0.5, 1.0));

            pos.push(Pnt3::new(vx+sc, vy_3, vz+sc)); nor.push(Vec3::new(0.0, 0.0, 1.0)); tex.push(Pnt2::new(1.0, 1.0));
            pos.push(Pnt3::new(vx+sc, 0.0, vz+sc)); nor.push(Vec3::new(0.0, 0.0, 1.0)); tex.push(Pnt2::new(1.0, 0.0));
            pos.push(Pnt3::new(vx, 0.0, vz+sc)); nor.push(Vec3::new(0.0, 0.0, 1.0)); tex.push(Pnt2::new(0.5, 0.0));

            pos.push(Pnt3::new(vx, 0.0, vz+sc)); nor.push(Vec3::new(0.0, 0.0, 1.0)); tex.push(Pnt2::new(0.5, 0.0));
            pos.push(Pnt3::new(vx, vy_1, vz+sc)); nor.push(Vec3::new(0.0, 0.0, 1.0)); tex.push(Pnt2::new(0.5, 1.0));
            pos.push(Pnt3::new(vx+sc, vy_3, vz+sc)); nor.push(Vec3::new(0.0, 0.0, 1.0)); tex.push(Pnt2::new(1.0, 1.0));
        }
    }

    (pos, nor, tex)
}
