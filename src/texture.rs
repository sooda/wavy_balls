use errors::*;

use glium;
use glium::backend::Facade;

use image;

use std::path::Path;

pub fn load_texture<F: Facade, P: AsRef<Path> + ?Sized>(facade: &F,
                                                        path: &P)
                                                        -> Result<glium::texture::Texture2d> {
    let image = image::open(path).chain_err(|| "failed to load image file")?.to_rgba();
    let dims = image.dimensions();

    let raw = glium::texture::RawImage2d::from_raw_rgba_reversed(image.into_raw(), dims);

    glium::texture::Texture2d::new(facade, raw).chain_err(|| "failed to load gpu texture")
}
