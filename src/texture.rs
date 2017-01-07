use errors::*;

use glium;
use glium::backend::Facade;

use image;

use std::path::Path;

pub fn load_texture<F: Facade, P: AsRef<Path> + ?Sized>(facade: &F,
                                                        path: &P)
                                                        -> Result<glium::texture::Texture2d> {
    let raw = self::load_image(path)?;

    glium::texture::Texture2d::new(facade, raw).chain_err(|| "failed to load texture to GPU")
}

pub fn load_image<P: AsRef<Path> + ?Sized>(path: &P)
                                           -> Result<glium::texture::RawImage2d<'static, u8>> {
    let image = image::open(path).chain_err(|| "failed to load image file")?.to_rgba();
    let dims = image.dimensions();

    Ok(glium::texture::RawImage2d::from_raw_rgba_reversed(image.into_raw(), dims))
}
