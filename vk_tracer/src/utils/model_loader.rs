use crate::errors::Result;
use crate::mesh::{MeshVertex, VertexXyz, VertexXyzUvNorm};
use crate::{MeshHandle, VkTracerApp};
use nalgebra_glm as glm;

pub trait GltfToVertex: MeshVertex + Sized {
    fn is_compatible(primitive: &gltf::Primitive) -> bool;
    fn from_gltf(primitive: &gltf::Primitive, buffers: &[gltf::buffer::Data]) -> Result<Vec<Self>>;
}

impl GltfToVertex for VertexXyz {
    fn is_compatible(primitive: &gltf::Primitive) -> bool {
        primitive.get(&gltf::Semantic::Positions).is_some()
    }

    fn from_gltf(primitive: &gltf::Primitive, buffers: &[gltf::buffer::Data]) -> Result<Vec<Self>> {
        Ok(primitive
            .reader(|b| Some(&buffers[b.index()]))
            .read_positions()
            .unwrap()
            .map(|pos| VertexXyz(glm::make_vec3(&pos)))
            .collect())
    }
}

impl GltfToVertex for VertexXyzUvNorm {
    fn is_compatible(primitive: &gltf::Primitive) -> bool {
        primitive.get(&gltf::Semantic::Positions).is_some()
            && primitive.get(&gltf::Semantic::TexCoords(0)).is_some()
            && primitive.get(&gltf::Semantic::Normals).is_some()
    }

    fn from_gltf(primitive: &gltf::Primitive, buffers: &[gltf::buffer::Data]) -> Result<Vec<Self>> {
        let reader = primitive.reader(|b| Some(&buffers[b.index()]));

        Ok(reader
            .read_positions()
            .unwrap()
            .zip(reader.read_tex_coords(0).unwrap().into_f32())
            .zip(reader.read_normals().unwrap())
            .map(|((pos, uv), norm)| VertexXyzUvNorm {
                xyz: glm::make_vec3(&pos),
                uv: glm::make_vec2(&uv),
                normal: glm::make_vec3(&norm),
            })
            .collect())
    }
}

impl VkTracerApp {
    pub fn load_first_mesh<V: GltfToVertex>(&mut self, filename: &str) -> Result<MeshHandle> {
        let (gltf, buffers, _) = gltf::import(filename)?;
        let primitive = gltf.meshes().nth(0).unwrap().primitives().nth(0).unwrap();
        assert!(V::is_compatible(&primitive));

        let vertices = V::from_gltf(&primitive, &buffers)?;
        let indices = {
            primitive
                .reader(|b| Some(&buffers[b.index()]))
                .read_indices()
                .unwrap()
                .into_u32()
                .collect::<Vec<_>>()
        };

        self.create_mesh_indexed(&vertices, &indices)
    }
}
