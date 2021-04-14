use crate::mesh::{Index, Mesh, Vertex, VertexPosUv};
use slotmap::{new_key_type, SlotMap};

new_key_type! {
    pub struct MeshId;
}

pub type StandardMeshStorage = MeshStorage<VertexPosUv, u16>;

pub struct MeshStorage<V: Vertex, I: Index> {
    storage: SlotMap<MeshId, Mesh<V, I>>,
}

impl<V: Vertex, I: Index> MeshStorage<V, I> {
    pub(crate) fn new() -> Self {
        Self {
            storage: SlotMap::with_key(),
        }
    }

    pub(crate) fn register_mesh(&mut self, mesh: Mesh<V, I>) -> MeshId {
        self.storage.insert(mesh)
    }

    pub(crate) fn get_mesh(&self, key: MeshId) -> Option<&Mesh<V, I>> {
        self.storage.get(key)
    }

    pub(crate) unsafe fn get_mesh_unchecked(&self, key: MeshId) -> &Mesh<V, I> {
        self.storage.get_unchecked(key)
    }
}
