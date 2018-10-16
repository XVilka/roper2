extern crate rand; 

use std::thread::{spawn, JoinHandle};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::cell::RefCell;
use std::sync::{Arc,RwLock}; 
use std::collections::VecDeque;

use self::rand::{Rng, SeedableRng};
use self::rand::isaac::Isaac64Rng;

use par::statics::*;
use gen::genotype::*;
use gen::phenotype::{Creature,Fitness};
use evo::crossover::{homologous_crossover};
use fit::circbuf::{CircBuf};

pub fn spawn_breeder(
    window_size: usize,
    rng_seed: RngSeed,
) -> (Sender<Creature>, Receiver<Creature>, JoinHandle<()>) {
    let (from_breeder_tx, from_breeder_rx) = channel();
    let (into_breeder_tx, into_breeder_rx) = channel();

    let sel_handle = spawn(move || {
      /* TODO */
    });

    (into_breeder_tx, from_breeder_rx, sel_handle)
}

fn xover_compat(c1: u64, c2: u64) -> usize {
      (c1 & c2).count_ones() as usize
  }
/* FOOBAR */

fn tournament(selection_window: &mut VecDeque<Creature>,
              seed: RngSeed) -> (RngSeed, Vec<Creature>) {
    let mut rng = Isaac64Rng::from_seed(seed);
    /* note: seed creation should probably be its own utility function */
    let mut new_seed: [u8; 32] = [0; 32];
    for i in 0..32 { new_seed[i] = rng.gen::<u8>() }

    assert!(*TSIZE as f32 * *MATE_SELECTION_FACTOR <= selection_window.len() as f32);
    let mut indices = rand::seq::sample_indices(&mut rng,
                                                selection_window.len(),
                                                (*TSIZE as f32 * *MATE_SELECTION_FACTOR)
                                                .floor() as usize);
    /* TODO: take n times as many combatants as needed, then winnow
     * out those least compatible with first combatant's crossover mask
     */
    let x1 = selection_window[0].genome.xbits;
    let xbit_vec : Vec<u64> = selection_window.iter().map(|c| c.genome.xbits).collect();
    let compatkey = |i: &usize| {
        let x2 = xbit_vec[*i];

        64 - xover_compat(x1, x2)
    };

    indices.sort_by_key(compatkey);
    /* now drop the least compatible from consideration */
    indices.truncate(*TSIZE);

    let fitkey = |i: &usize| {
      0 /* FIXME */
    };
    /* now, sort the remaining indices by the fitness of their creatures */
    /* TODO -- we need a pareto sorting function */
    indices.sort_by_key(fitkey);
    /* and choose the parents and the fallen */
    // *TSIZE must be >= 4.
    let (p0, p1) = (indices[*TSIZE-1], indices[*TSIZE-2]);
    let (d0, d1) = (indices[0], indices[1]);

 /* I think I need to have the selection window consist of refcells of creatures, 
    instead of just naked creatures */ /* FIXME */ 
    let (mother, father) = (&selection_window[p0], &selection_window[p1]);
    let offspring = homologous_crossover(mother, father, &mut rng);
    /* now, place the offspring back in the population by inserting them
     * into the selection window
     */

    (new_seed, offspring)
}
