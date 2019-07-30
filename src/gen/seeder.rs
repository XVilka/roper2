use std::sync::Arc;
use std::sync::mpsc::{sync_channel, Receiver};
use std::thread::{spawn, JoinHandle};

use rand::{SeedableRng,Rng};
use rand_isaac::isaac64::Isaac64Rng;

use crate::genotype::*;
use crate::phenotype::*;
use crate::par::statics::*;

pub fn new_creature<R: Rng>(rng: &mut R,
                            problem_set: &Vec<Vec<u64>>,
                            index: usize) -> Creature {
    /* create a Creature::from_seed function */
    let len_range = (*MIN_CREATURE_LENGTH, *MAX_CREATURE_LENGTH);
    let genome = Chain::from_seed(rng, len_range);
    let mut creature = Creature::new(genome, index);
    for problem in problem_set.iter() {
        creature.pose_problem(&problem);
    }
    /* Clearly nothing should have hatched yet */
    assert!(!creature.has_hatched());
    creature
}

pub fn spawn_seeder(
    num_wanted: usize,
    problem_set: &Vec<Vec<u64>>,
) -> (Receiver<Creature>, JoinHandle<()>)
{
    println!("[+] Spawning seeder");
    let seed = RNG_SEED.clone();
    let (from_seeder_tx, from_seeder_rx) = sync_channel(*CHANNEL_SIZE);
    //    let (into_seeder_tx, into_seeder_rx) = channel();
    let problem_set = Arc::new(problem_set.clone());
    let seeder_handle = spawn(move || {
        let problem_set = problem_set.clone();
        let mut rng = Isaac64Rng::from_seed(seed);
        let mut index = 0;
        while index < num_wanted {
            let creature = new_creature(&mut rng, &problem_set, index);
            index += 1;
            match from_seeder_tx.send(creature) {
                Ok(_) => (),
                Err(_) => println!("[+] Sending error in seeder at index = {}", index),
            }
        }
    });
    (from_seeder_rx, seeder_handle)
}
