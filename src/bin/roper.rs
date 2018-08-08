extern crate libroper;
extern crate rand;
extern crate unicorn;

use rand::{thread_rng, Rng};
use std::env;
use std::time::Instant;
use std::sync::mpsc::Sender;

use libroper::gen::*;
use libroper::{emu, gen, log};
use libroper::evo::pipeline;
use libroper::{evo,fit};
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

fn seeder_hatchery_pipeline(engines: usize, expect: usize, logger_tx: Sender<Creature>) {
    let start = Instant::now();
    let num_evals = engines;
    let (seed_rx, seed_hdl) = gen::spawn_seeder(
        expect,
        (2, 32),
        &vec![vec![1, 2, 3]],
        mkseed(start.elapsed().subsec_nanos() as u64),
    );
    let (hatch_tx, hatch_rx, hatch_hdl) = emu::spawn_hatchery(engines, expect);
    let (eval_tx, eval_rx, eval_hdl) = fit::spawn_evaluator(num_evals, 512);
    // let (sel_tx, sel_rx, sel_hdl) = emu::spawn_breeder(); // ?
    let pipe_hdl_1 = pipeline(seed_rx, vec![hatch_tx, logger_tx.clone()]);

    /* KLUDGEY TESTING THING */
    let p0 = hatch_rx.recv().unwrap();
    let p1 = hatch_rx.recv().unwrap();
    let mut rng = thread_rng(); /* screw it, this is just a test */
    let _offspring = evo::crossover::homologous_crossover(&p0, &p1, &mut rng);

    //println!("hello");
    let pipe_hdl_2 = pipeline(hatch_rx, vec![eval_tx]);
    let pipe_hdl_3 = pipeline(eval_rx, vec![logger_tx]);

    seed_hdl.join().unwrap(); //println!("seed_hdl joined");
    hatch_hdl.join().unwrap(); //println!("hatch_hdl joined");
    eval_hdl.join().unwrap();
    pipe_hdl_1.join().unwrap(); //println!("pipe_hdl_1 joined.");
    pipe_hdl_2.join().unwrap(); //println!("pipe_hdl_2 joined");
    pipe_hdl_3.join().unwrap(); //println!("pipe_hdl_3 joined");
    let elapsed = start.elapsed();
    println!(
        "{} {} {}",
        expect,
        engines,
        elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 / 1000000000.0
    );
}

fn main() {
    let _dummy = unicorn::CpuX86::new(unicorn::Mode::MODE_64).unwrap();
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
    let log_freq = if cfg!(debug_assertions) { 4096 } else { 999999 };
    let (logger_tx, logger_hdl) = log::spawn_logger(512, log_freq);
    for _ in 0..loops {
        seeder_hatchery_pipeline(engines, expect, logger_tx.clone());
    }
    drop(logger_tx);
    logger_hdl.join().unwrap();
}
