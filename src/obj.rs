use math::*;
use errors::*;

use na::Norm;

use std::path::Path;
use std::fs::File;
use std::io::{BufRead, BufReader};

pub fn load_obj<P: AsRef<Path> + ?Sized>(p: &P) -> Result<(Vec<Pnt3>, Vec<Vec3>, Vec<Pnt3>)> {
    fn parse_f(tok: &str) -> Result<(isize, Option<isize>, Option<isize>)> {
        let s = tok.split('/').collect::<Vec<_>>();
        Ok(match s.len() {
            1 => (tok.parse::<isize>().unwrap() - 1, None, None),
            3 => {
                (s[0].parse::<isize>().unwrap() - 1,
                 s[1].parse::<isize>().ok().map(|i| i - 1),
                 s[2].parse::<isize>().ok().map(|i| i - 1))
            }
            _ => bail!(ErrorKind::ObjLoadError),
        })
    }

    let mut obj_verts = vec![];
    let mut obj_norms = vec![];
    let mut obj_texcs = vec![];
    let mut tris = vec![];
    let mut norms = vec![];
    let mut texcs = vec![];

    for line in BufReader::new(File::open(p).unwrap()).lines() {
        let line = line.unwrap();
        let toks = line.split_whitespace().collect::<Vec<_>>();

        if toks[0] == "v" {
            obj_verts.push(Pnt3::new(toks[1].parse().unwrap(),
                                     toks[2].parse().unwrap(),
                                     toks[3].parse().unwrap()));
        } else if toks[0] == "vn" {
            obj_norms.push(Vec3::new(toks[1].parse().unwrap(),
                                     toks[2].parse().unwrap(),
                                     toks[3].parse().unwrap())
                .normalize());
        } else if toks[0] == "vt" {
            let w = toks[2].parse().unwrap_or(0.0);
            obj_texcs.push(Pnt3::new(toks[1].parse().unwrap(), toks[2].parse().unwrap(), w))
        } else if toks[0] == "f" {
            let (a, b, c) = (parse_f(toks[1])?, parse_f(toks[2])?, parse_f(toks[3])?);

            let v_i = |i| if i < 0 {
                (obj_verts.len() as isize + i + 1) as usize
            } else {
                i as usize
            };
            let n_i = |i| if i < 0 {
                (obj_norms.len() as isize + i + 1) as usize
            } else {
                i as usize
            };
            let t_i = |i| if i < 0 {
                (obj_texcs.len() as isize + i + 1) as usize
            } else {
                i as usize
            };

            let tri = (obj_verts[v_i(a.0)], obj_verts[v_i(b.0)], obj_verts[v_i(c.0)]);
            let n;
            if a.2.is_none() {
                let u = tri.1 - tri.0;
                let v = tri.2 - tri.0;
                let pn = Vec3::new(u.y * v.z - u.z * v.y,
                                   u.z * v.x - u.x * v.z,
                                   u.x * v.y - u.y * v.x)
                    .normalize();
                n = (pn, pn, pn);
            } else {
                let na = obj_norms[n_i(a.2.unwrap())];
                let nb = obj_norms[n_i(b.2.unwrap())];
                let nc = obj_norms[n_i(c.2.unwrap())];
                n = (na, nb, nc);
            }
            let texc;
            if a.1.is_none() {
                texc = (Pnt3::new(0.0, 0.0, 0.0),
                        Pnt3::new(0.0, 0.0, 0.0),
                        Pnt3::new(0.0, 0.0, 0.0));
            } else {
                texc = (obj_texcs[t_i(a.1.unwrap())],
                        obj_texcs[t_i(b.1.unwrap())],
                        obj_texcs[t_i(c.1.unwrap())]);
            }

            tris.push(tri.0);
            tris.push(tri.1);
            tris.push(tri.2);
            norms.push(n.0);
            norms.push(n.1);
            norms.push(n.2);
            texcs.push(texc.0);
            texcs.push(texc.1);
            texcs.push(texc.2);
        }
    }

    Ok((tris, norms, texcs))
}
