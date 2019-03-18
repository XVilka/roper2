extern crate libroper;
extern crate rand;
extern crate unicorn;

use rand::{thread_rng, Rng};
use std::env;
use std::time::Instant;
use std::sync::mpsc::Sender;

use libroper::gen::*;
use libroper::{emu, gen, log};
use libroper::evo::*;
use libroper::evo::evolver::*;
use libroper::{evo,fit};
use libroper::par::statics::*;
/* The optimal combination, so far, seems to be something like:
 * batch of 1024, channels throttled to 512, number of engines: 4-6
 * 0.09 seconds to evaluate 1024 specimens!
 */

fn mkseed(u: u64) -> [u8; 32] {
    let mut seed = [0u8; 32];
    for i in 0..32 {
        seed[i] = (u.rotate_left(2) & 0xFF) as u8;
    }
    seed
}

/*
fn seeder_hatchery_pipeline(engines: usize) {
    println!("[>] seeder_hatchery_pipeline");
    let num_evals = engines;

    /* Spawn the components */
    let rng_seed = *RNG_SEED;
    println!("[>] spawning seeder");
    let (seed_rx, seed_hdl) = gen::spawn_seeder(
        0x200000, /* pop size */
        (2, 32),
        &vec![vec![1,2]], /* fake problem set */
        &rng_seed,
    );
    /* consider a shuffling component. it will collect N creatures,
    shuffle them, then re-send them in a new, random order. if the window
    is relatively prime with the population size, that should keep everything
    moving around a bit more. */
    println!("[>] spawning logger");
    let (logger_tx, logger_hdl) = log::spawn_logger(0x2001, 0x2000);
    println!("[>] spawning hatchery");
    let (hatch_tx, hatch_rx, hatch_hdl) = emu::spawn_hatchery(engines);
    println!("[>] spawning evaluator");
    let (eval_tx, eval_rx, eval_hdl) = fit::spawn_evaluator(num_evals, 2048);
    println!("[>] spawning breeder");
    let (breed_tx, breed_rx, sel_hdl) = evo::spawn_breeder(65); // ?

    /* Build the pipelines */
    let pipe_hdl_1 = pipeline(seed_rx, vec![hatch_tx.clone()], 0, "seed/hatch");
    let pipe_hdl_2 = pipeline(hatch_rx, vec![eval_tx], 0, "hatch/eval");
    let pipe_hdl_3 = pipeline(eval_rx,  vec![logger_tx, breed_tx], 0, "eval/log,breed");
    let pipe_hdl_4 = pipeline(breed_rx, vec![hatch_tx.clone()], 0, "breed/hatch");
    //let pipe_hdl_3 = pipeline(eval_rx, vec![hatch_tx.clone()], 0, "eval/hatch");


    seed_hdl.join().unwrap(); //println!("seed_hdl joined");
    hatch_hdl.join().unwrap(); //println!("hatch_hdl joined");
    eval_hdl.join().unwrap();
    sel_hdl.join().unwrap();
    logger_hdl.join().unwrap();
    pipe_hdl_1.join().unwrap(); //println!("pipe_hdl_1 joined.");
    pipe_hdl_2.join().unwrap(); //println!("pipe_hdl_2 joined");
    pipe_hdl_3.join().unwrap(); //println!("pipe_hdl_3 joined");
    //pipe_hdl_4.join().unwrap();
}
*/

fn main() {
    let engines = match env::var("ROPER_ENGINES") {
        Err(_) => if cfg!(debug_assertions) {
            1
        } else {
            4
        },
        Ok(n) => n.parse::<usize>()
            .expect("Failed to parse ROPER_ENGINES env var"),
    };
    let expect = match env::var("ROPER_STRESS_LOAD") {
        Err(_) => 1024,
        Ok(n) => n.parse::<usize>()
            .expect("Failed to parse ROPER_STRESS_EXPECT"),
    };
    let loops = match env::var("ROPER_LOOPS") {
        Err(_) => 1024,
        Ok(n) => n.parse::<usize>().expect("Failed to parse ROPER_LOOPS"),
    };
    //let mut rng = Isaac64Rng::from_seed(&RNG_SEED);

    //let (log_tx,log_handle) = log::spawn_logger(0x1000);
    /*
    for counter in 0..loops {
        do_the_thing(engines, expect, &mut rng, counter, &log_tx);
    }
    */
    //drop(log_tx);
    //log_handle.join().unwrap();
    //seeder_hatchery_pipeline(engines);
    evolution_pond();
}

/* we need a pool for creatures to rest in, without madly circulating through
the channels when they don't need to be. */
