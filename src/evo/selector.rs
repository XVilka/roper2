extern crate rand; 

use std::thread::{spawn, JoinHandle};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::cell::RefCell;
use std::sync::{Arc,RwLock}; 
use std::collections::VecDeque;

use self::rand::{Rng, SeedableRng};
use self::rand::isaac::Isaac64Rng;

use par::statics::*;
use gen::genotype::*;
use gen::phenotype::{FitFuncs,Creature,Fitness,FitnessOps,Phenome};
use evo::crossover::{homologous_crossover};
use fit::circbuf::{CircBuf};



pub fn spawn_breeder(
    window_size: usize,
    hatch_tx: &SyncSender<Creature>,
) -> (SyncSender<Creature>, Receiver<Creature>, JoinHandle<()>) {
    let hatch_tx = hatch_tx.clone();
    let (from_breeder_tx, from_breeder_rx) = sync_channel(*CHANNEL_SIZE);
    let (into_breeder_tx, into_breeder_rx) = sync_channel(*CHANNEL_SIZE);
    let mut rng_seed = RNG_SEED.clone();
    let sel_handle = spawn(move || {
        /* TODO */
        let mut sel_window: Vec<Creature> = Vec::with_capacity(window_size);
        for incoming in into_breeder_rx {
            /* STUB, because the spice must flow */
            let incoming : Creature = incoming;
            //if incoming.generation() > 1 {
            //    println!("[!] Gen {} incoming!\n{}", incoming.generation(), incoming);
            //}
            sel_window.push(incoming);
            if sel_window.len() >= window_size {
                // causing SendError on eval/log,breed //
                let mut offspring = tournament(&mut sel_window, rng_seed);
                while sel_window.len() > 0 {
                    match sel_window.pop() {
                        Some(outgoing) => {
                            match from_breeder_tx.send(outgoing) {
                                Ok(_) => (),
                                Err(e) => println!("Error sending to from_breeder_tx: {:?}", e),
                            }
                        },
                        None => panic!("unreachable?"),
                    }
                }
                while offspring.len() > 0 {
                    match offspring.pop() {
                        Some(outgoing) => {
                            match hatch_tx.send(outgoing) {
                                Ok (_) => (),
                                Err(e) => println!("Error sending to hatch_tx: {:?}",e),
                            }
                        },
                        None => panic!("unreachable??"),
                    }
                }
            }
        }
    });

    (into_breeder_tx, from_breeder_rx, sel_handle)
}

fn xover_compat(c1: u64, c2: u64) -> usize {
      (c1 & c2).count_ones() as usize
  }
/* FOOBAR */

fn tournament(selection_window: &mut Vec<Creature>,
              seed: RngSeed) -> Vec<Creature> {
    let mut rng = Isaac64Rng::from_seed(seed);
    /* note: seed creation should probably be its own utility function */
    let mut new_seed: [u8; 32] = [0; 32];
    for i in 0..32 { new_seed[i] = rng.gen::<u8>() }


    if (*TSIZE as f32 * *MATE_SELECTION_FACTOR > selection_window.len() as f32) {
        println!("TSIZE = {}; MATE_SELECTION_FACTOR = {}; selection_window.len() = {}",
                 *TSIZE, *MATE_SELECTION_FACTOR, selection_window.len());
        panic!("aarggh");
    };
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

  // comment to simply disable compatibility sorting
    indices.sort_by_key(compatkey);
    /* now drop the least compatible from consideration */
    indices.truncate(*TSIZE);

    let mut max_val = 0;
    {
        let fitkey = |i: &usize| {
            let i = *i;
            let result = 
                (&selection_window)[i]
                .fitness
                .as_ref()
                .unwrap() /* FIXME bluff for debugging */
                .mean();
            if result > max_val { max_val = result };
            result
        };
        /* now, sort the remaining indices by the fitness of their creatures */
        /* TODO -- we need a pareto sorting function */
        indices.sort_by_key(fitkey);
        indices.reverse();
    }
    /* and choose the parents and the fallen */
    // *TSIZE must be >= 4.
    /* The dead */
    let (d0, d1) = (indices[*TSIZE-1], indices[*TSIZE-2]);
    /* The parents */
    let (p0, p1) = (indices[0], indices[1]);




 /* I think I need to have the selection window consist of refcells of creatures, 
    instead of just naked creatures */ /* FIXME */ 
    let mut offspring;
    {
        let mother = &selection_window[p0];
        let father = &selection_window[p1];
        //let dead0  = &selection_window[d0];
        //let dead1  = &selection_window[d1];
        //println!("** mother.fitness = {:?}; father.fitness = {:?}; dead0.fitness = {:?}; dead1.fitness = {:?}",
        //         mother.fitness, father.fitness, dead0.fitness, dead1.fitness);
        offspring = homologous_crossover(mother, father, &mut rng);
        offspring[0].inherit_problems(&father);
        offspring[1].inherit_problems(&father);
    }
    /* now, place the offspring back in the population by inserting them
     * into the selection window
     */
    //selection_window[d0] = offspring.pop().unwrap();
    //selection_window[d1] = offspring.pop().unwrap();
    /* It's essential that we remove the higher index first */
    assert!(d0 != d1);
    let first_to_kill = usize::max(d0, d1);
    let next_to_kill = usize::min(d0, d1);
    selection_window.swap_remove(first_to_kill);
    selection_window.swap_remove(next_to_kill);
    offspring
}
