extern crate libroper;
extern crate rand;
extern crate unicorn;
use libroper::evo::evolver::evolution_pond;

fn main() {
    evolution_pond();
}

/* we need a pool for creatures to rest in, without madly circulating through
the channels when they don't need to be. */
