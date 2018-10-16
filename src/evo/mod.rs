pub mod evolver;
pub use evolver::pipeline;

pub mod selector;
pub use selector::spawn_breeder;
pub mod crossover;
pub use crossover::homologous_crossover;
