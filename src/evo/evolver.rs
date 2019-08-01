use std::thread::{spawn, JoinHandle};
use std::sync::mpsc::{Receiver, SyncSender};
use rand::{SeedableRng};
use rand::seq::SliceRandom;
use rand_isaac::isaac64::Isaac64Rng;

use crate::emu;
use crate::fit;
use crate::gen;
use crate::gen::Creature;
use crate::log;
use crate::par::statics::*;
use crate::selector::*;


/* The genotype->phenotype pipeline */
/* -- spawns hatchery
 * -- receives new genotypes from initialization and/or breeding
 * -- sends new genotypes to hatchery, receives phenotypes
 * -- sends phenotypes to selection routine
 * -- selection sends some genotypes of those phenotypes to reproduction routine
 * -- reproduction routine sends them back here, to go on to hatchery
 */

pub fn pipeline(rx: Receiver<Creature>, tx_refs: Vec<&SyncSender<Creature>>,
                limit: usize, note: &'static str) -> JoinHandle<()> {
    let mut txs : Vec<SyncSender<Creature>> = Vec::new();
    for tx_ref in tx_refs.into_iter() {
        txs.push(tx_ref.clone());
    }
    spawn(move || {
        let mut count = 0;
        for x in rx {
            count += 1;
            if limit == 0 || count < limit {
                if txs.len() > 1 {
                    let mut tx_num = 1;
                    for tx in txs[1..].iter() {
                        match tx.send(x.clone()) {
                            Err(e) => println!("[tx:{}] {}: {:?}", tx_num, note, e),
                            Ok(_k) =>  (), //println!("[tx:{}] {} ok {:?}", tx_num, note, _k),
                        }
                        tx_num += 1;
                    }
                };
                match txs[0].send(x) {
                    Err(e) => {
                        println!("[tx:0] {}: {:?}", note, e);
                        std::process::exit(99);
                    },
                    Ok(_k) =>  (), //println!("[tx:0] {} ok {:?}", note, _k),
                }
            } else {
                println!("[!] Limit of {} on {} pipeline reached. Concluding.", limit, note);
                for tx in txs.iter() {
                    drop(tx)
                }
                std::process::exit(0);

            }
        }
    })
}


#[allow(unused_variables)]
pub fn evolution_pond() {

    let rng_seed = *RNG_SEED;
    let mut rng = Isaac64Rng::from_seed(rng_seed);

    println!("[>] spawning seeder");
    let (seed_rx, seed_hdl) = gen::spawn_seeder(
        *POPULATION_SIZE,
        &vec![vec![1,2]], /* fake problem set */
    );

//    let (refill_pond_tx, refill_pond_rx) = sync_channel(*CHANNEL_SIZE);

    println!("[>] spawning logger");
    let (logger_tx, logger_hdl) = log::spawn_logger(*POPULATION_SIZE/10, *POPULATION_SIZE/10);
    println!("[>] spawning hatchery");
    let (hatch_tx, hatch_rx, hatch_hdl) = emu::spawn_hatchery(*NUM_ENGINES);
    println!("[>] spawning evaluator");
    let (eval_tx, eval_rx, eval_hdl) = fit::spawn_evaluator(*NUM_ENGINES, 2048);
    println!("[>] spawning breeder");
    let (breed_tx, breed_rx, sel_hdl) = spawn_breeder(*SELECTION_WINDOW_SIZE,
                                                      &hatch_tx); // ?


    let seed_hatch_pipe = pipeline(seed_rx, vec![&hatch_tx], 0, "seed/hatch");
    let hatch_eval_pipe = pipeline(hatch_rx, vec![&eval_tx], 0, "hatch/eval");
    let eval_breed_pipe = pipeline(eval_rx, vec![&breed_tx,
                                                 &logger_tx],
                                   0, "eval/breed+log");


    let mut pond : Vec<Creature> = Vec::new();

    /* Initialize the pond with already hatched and evaluated creatures */
    let mut count = 0;
    for critter in breed_rx.iter() {
        pond.push(critter);
        count += 1;
        //println!("count {}",count);
        if pond.len() > *SELECTION_WINDOW_SIZE {
            pond.shuffle(&mut rng);
            /* TODO: get random indices, then use remove_swap instead */
            for i in 0..(*SELECTION_WINDOW_SIZE) {
                match pond.pop() {
                    Some (critter) => {
                        let res =
                            if critter.has_hatched() {
                                breed_tx.send(critter)
                            } else {
                                hatch_tx.send(critter)
                            };
                        match res {
                            Ok(_) => (),
                            Err(e) => println!("error {:?}", e),
                        }
                    },
                    None => println!("No critters"),
                }
            }
        }
    }

    println!("[+] Population initialized.");


    /* safety net */
    loop {}

}

/* The phenotype->genotype pipeline */
