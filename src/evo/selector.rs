use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread::{spawn, JoinHandle};

use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_isaac::isaac64::Isaac64Rng;

use crate::evo::crossover::homologous_crossover;
use crate::gen::phenotype::{Creature, FitnessOps};
use crate::par::statics::*;

pub fn spawn_breeder(
    window_size: usize,
    hatch_tx: &SyncSender<Creature>,
) -> (SyncSender<Creature>, Receiver<Creature>, JoinHandle<()>) {
    let hatch_tx = hatch_tx.clone();
    let (from_breeder_tx, from_breeder_rx) = sync_channel(*CHANNEL_SIZE);
    let (into_breeder_tx, into_breeder_rx) = sync_channel(*CHANNEL_SIZE);
    let rng_seed = *RNG_SEED;
    let sel_handle = spawn(move || {
        /* TODO */
        let mut sel_window: Vec<Creature> = Vec::with_capacity(window_size);
        for incoming in into_breeder_rx {
            /* STUB, because the spice must flow */
            let incoming: Creature = incoming;
            //if incoming.generation() > 1 {
            //    println!("[!] Gen {} incoming!\n{}", incoming.generation(), incoming);
            //}
            sel_window.push(incoming);
            if sel_window.len() >= window_size {
                // causing SendError on eval/log,breed //
                let mut offspring = tournament(&mut sel_window, rng_seed);
                while !sel_window.is_empty() {
                    match sel_window.pop() {
                        Some(outgoing) => match from_breeder_tx.send(outgoing) {
                            Ok(_) => (),
                            Err(e) => println!("Error sending to from_breeder_tx: {:?}", e),
                        },
                        None => panic!("unreachable?"),
                    }
                }
                while !offspring.is_empty() {
                    match offspring.pop() {
                        Some(outgoing) => match hatch_tx.send(outgoing) {
                            Ok(_) => (),
                            Err(e) => println!("Error sending to hatch_tx: {:?}", e),
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

fn tournament(selection_window: &mut Vec<Creature>, seed: RngSeed) -> Vec<Creature> {
    let mut rng = Isaac64Rng::from_seed(seed);
    /* note: seed creation should probably be its own utility function */
    let mut new_seed: [u8; 32] = [0; 32];
    for i in 0..32 {
        new_seed[i] = rng.gen::<u8>()
    }

    if *TSIZE as f32 * *MATE_SELECTION_FACTOR > selection_window.len() as f32 {
        println!(
            "TSIZE = {}; MATE_SELECTION_FACTOR = {}; selection_window.len() = {}",
            *TSIZE,
            *MATE_SELECTION_FACTOR,
            selection_window.len()
        );
        panic!("aarggh");
    };
    let mut indices = rand::seq::index::sample(
        &mut rng,
        selection_window.len(),
        (*TSIZE as f32 * *MATE_SELECTION_FACTOR).floor() as usize,
    )
    .into_vec();
    /* TODO: take n times as many combatants as needed, then winnow
     * out those least compatible with first combatant's crossover mask
     */
    let x1 = selection_window[0].genome.xbits;
    let xbit_vec: Vec<u64> = selection_window.iter().map(|c| c.genome.xbits).collect();
    let compatkey = |i: &usize| {
        let x2 = xbit_vec[*i];

        64 - xover_compat(x1, x2)
    };

    // comment to simply disable compatibility sorting
    indices.sort_by_key(compatkey);
    /* now drop the least compatible from consideration */
    indices.truncate(*TSIZE);

    {
        let fitkey = |i: &usize| {
            let i = *i;
            let result = (&selection_window)[i]
                .fitness
                .as_ref()
                .unwrap() /* FIXME bluff for debugging */
                .mean();
            (result * 10000.00) as usize
        };
        /* now, sort the remaining indices by the fitness of their creatures */
        /* TODO -- we need a pareto sorting function */
        indices.sort_by_key(fitkey);
        indices.reverse();
    }
    /* TODO: Pareto ranking */
    /*
    Instead of /sorting/ the indices, filter or partition them according to
    pareto dominance. The filter function would look like this:

    (Naive implementation -- needs to be optimized, since it looks quadratic)
     */
    let mut pareto_front: Vec<usize>;
    {
        /* block to contain immutable borrow of the window */
        let pareto_filter = |index: &&usize| {
            let fvec = &selection_window[**index].fitness.as_ref().unwrap();
            /* fvec is dominated if there exists another fitness vector
            fvec2 in selection_window such that fvec2[i] > fvec[i] for
            all i under fvec.len() */
            let len = fvec.len();
            let num_dominators = indices
                .iter()
                .filter(|i| {
                    if i == index {
                        return false;
                    };
                    let c = &selection_window[**i];
                    let fvec_d = c.fitness.as_ref().unwrap();
                    let mut result = true;
                    for i in 0..len {
                        if !result {
                            break;
                        };
                        if fvec[i] >= fvec_d[i] {
                            //  println!("[PARETO]=[{}]=> {:?} >= {:?}", index, fvec[i], fvec_d[i]);
                            result = false;
                        } else {
                            //println!("[PARETO]=[{}]=> {:?} <  {:?}", index, fvec[i], fvec_d[i]);
                        }
                    }
                    /*
                    if result {
                       println!("[PARETO]=[{}]=> fvec {:?} is DOMINATED by fvec_d {:?}", index, fvec, fvec_d);
                    } else {
                       println!("[PARETO]=[{}]=> fvec {:?} is not dominated by fvec_d {:?}", index, fvec, fvec_d);
                    }
                    */
                    result
                })
                .count();
            //println!("[PARETO] num_dominators of {:?} == {}", fvec, num_dominators);
            num_dominators == 0 /* return true only for the pareto front */
        };
        pareto_front = indices
            .iter()
            .filter(pareto_filter)
            .copied()
            .collect::<Vec<usize>>();
        /*
                 let for_show = indices.iter() .map(|i| ((selection_window[*i]).fitness.as_ref().unwrap().clone())) .collect::<Vec<Vec<usize>>>();
                 println!("[PARETO] pareto_filter results: {:?}",
                          indices.iter().map(|i| (*i, pareto_filter(&i))).collect::<Vec<(usize,bool)>>());
                 println!("[PARETO] fvecs: {:?}", for_show);
                 println!("[PARETO] indices = {:?}", indices);
                 println!("[PARETO] Front: {:?}", pareto_front);
        */
        pareto_front.shuffle(&mut rng);
    }
    /*

    */
    /* and choose the parents and the fallen */
    // *TSIZE must be >= 4.
    /* The dead */
    /* The parents */
    assert!(!pareto_front.is_empty());
    let (p0, p1): (usize, usize);
    p0 = pareto_front[0];
    if pareto_front.len() >= 2 {
        p1 = pareto_front[1];
    } else {
        p1 = indices[0];
    }
    //let (d0, d1) = (indices[*TSIZE-1], indices[*TSIZE-2]);
    /* Consider filtering against the pareto front instead of the parent pair */
    let mut dead_meta_idx = *TSIZE - 1;
    assert!(*TSIZE > 2);
    while dead_meta_idx > 0 && (indices[dead_meta_idx] == p0 || indices[dead_meta_idx] == p1) {
        dead_meta_idx -= 1;
    }
    let d0 = indices[dead_meta_idx];
    dead_meta_idx -= 1;
    while dead_meta_idx > 0 && (indices[dead_meta_idx] == p0 || indices[dead_meta_idx] == p1) {
        dead_meta_idx -= 1;
    }
    let d1 = indices[dead_meta_idx];

    assert!(p0 != d0);
    assert!(p0 != d1);
    assert!(p1 != d0);
    assert!(p1 != d1);
    //let (p0, p1) = (indices[0], indices[1]);

    /* I think I need to have the selection window consist of refcells of creatures,
    instead of just naked creatures */
    /* FIXME */
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
