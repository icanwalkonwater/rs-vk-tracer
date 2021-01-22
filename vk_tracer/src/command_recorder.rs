#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum QueueType {
    Graphics,
    Transfer,
    Present,
}
