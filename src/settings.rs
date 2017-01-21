use errors::*;
use math::*;

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};

// bleh, would include rows: Vec<String> and a HashMap<&str, Vec<&str>> but the refs wouldn't
// probably work, right?
pub struct Settings {
    items: HashMap<String, Vec<String>>,
}

impl Settings {
    pub fn new(filename: &str) -> Result<Self> {
        let mut items: HashMap<String, Vec<String>> = HashMap::new();

        for line in BufReader::new(File::open(filename).chain_err(|| "cannot open file")?).lines() {
            let line = line.unwrap();
            let line = line.split('#').next().unwrap();
            let tokens = line.split_whitespace().collect::<Vec<_>>();
            if tokens.len() > 0 {
                let (name, rest) = tokens.split_at(1);
                let rest = rest.iter().map(|&x| x.to_owned()).collect();
                items.insert(name[0].to_owned(), rest);
            }
        }

        Ok(Settings { items: items })
    }

    pub fn get_u32(&self, name: &str) -> u32 {
        self.items.get(name).unwrap()[0].parse().unwrap()
    }

    pub fn get_f32(&self, name: &str) -> f32 {
        self.items.get(name).unwrap()[0].parse().unwrap()
    }

    pub fn get_vec3(&self, name: &str) -> Vec3 {
        let mut v = self.items.get(name).unwrap().iter().map(|x| x.parse().unwrap());
        Vec3::new(v.next().unwrap(), v.next().unwrap(), v.next().unwrap())
    }
}
