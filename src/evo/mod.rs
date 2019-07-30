pub mod evolver;
pub use crate::evolver::pipeline;

pub mod selector;
pub use crate::selector::spawn_breeder;
pub mod crossover;
pub use crate::crossover::homologous_crossover;
