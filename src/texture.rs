use errors::*;

use glium;
use glium::backend::Facade;
use glium::texture::{RawImage2d, Texture2d, Texture2dArray};
use glium::uniforms::{UniformValue, AsUniformValue};

use image;

use std::path::Path;

pub enum Texture {
    Twod(Texture2d),
    Array(Texture2dArray),
}

impl<'a> AsUniformValue for &'a Texture {
    fn as_uniform_value(&self) -> glium::uniforms::UniformValue {
        use self::Texture::*;
        match **self {
            Twod(ref t) => UniformValue::Texture2d(t, None),
            Array(ref t) => UniformValue::Texture2dArray(t, None),
        }
    }
}

pub fn load_texture<F: Facade, P: AsRef<Path> + ?Sized>(facade: &F, path: &P) -> Result<Texture> {
    let raw = self::load_image(path)?;

    glium::texture::Texture2d::new(facade, raw)
        .map(|t| Texture::Twod(t))
        .chain_err(|| "failed to load texture to GPU")
}

pub fn _load_texture_array<F: Facade, P: AsRef<Path> + ?Sized>(facade: &F,
                                                               paths: &[&P])
                                                               -> Result<Texture> {
    let raw: Result<Vec<RawImage2d<'static, u8>>> = paths.iter().map(|&p| load_image(p)).collect();
    let raw = raw?;

    glium::texture::Texture2dArray::new(facade, raw)
        .map(|t| Texture::Array(t))
        .chain_err(|| "failed to load texture to GPU")
}

pub fn load_image<P: AsRef<Path> + ?Sized>(path: &P) -> Result<RawImage2d<'static, u8>> {
    let image = image::open(path).chain_err(|| "failed to load image file")?.to_rgba();
    let dims = image.dimensions();

    Ok(glium::texture::RawImage2d::from_raw_rgba_reversed(image.into_raw(), dims))
}
