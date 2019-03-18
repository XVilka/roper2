use std::thread::{spawn, JoinHandle};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, RwLock};

use gen::{Creature,FitFuncs,FitnessOps,Fitness};
use fit::CircBuf;
use par::statics::*;

/* the statistical functions can be defined as methods on
 * CircBuf
 */

/// The logger sits at the receiving end of a one-way channel.
/// It's best to send cloned data to it, since you won't get it back.
pub fn spawn_logger(circbuf_size: usize, log_freq: usize) -> (SyncSender<Creature>, JoinHandle<()>) {
    println!("Logger spawned. Send clones!");
    let (log_tx, log_rx) = sync_channel(*CHANNEL_SIZE * 10);

    let circbuf = Arc::new(RwLock::new(CircBuf::new(circbuf_size)));

    let (analyse_tx, analyse_rx) = sync_channel(*CHANNEL_SIZE);

    let window = circbuf.clone();
    let _stat_handle = spawn(move || {
        let mut max_fit = 0.0;
        let mut max_gen = 0;
        for _ in analyse_rx {
            let window = window.read().unwrap();
            /* TODO here is where the analyses will be dispatched from */
            //println!("circbuf holds {}", window.buf.len());
            let mut sum_fit = 0.0;
            let mut sum_gen = 0.0;
            let mut sum_len = 0;
            let mut count = 0;
            for creature in window.buf.iter() {
                assert!(creature.has_hatched());
                match &creature.fitness {
                    &None => panic!("-- creature with no fitness in logger"),
                    &Some (ref fvec) => {
                        count += 1;
                        let fit = fvec.mean() as f32;
                        if fit > max_fit {
                            println!("[LOGGER] Fitness: {}\n{}", fit, &creature);
                            max_fit = fit;
                        };
                        sum_fit += fit;
                        let gen = creature.generation();
                        if gen > max_gen { max_gen = gen };
                        sum_gen += gen as f32;
                        let len = creature.genome.len();
                        sum_len += len;
                    },
                }
            }
            let mean_fitness = sum_fit / count as f32;
            let mean_gen = sum_gen / count as f32;
            let mean_len = sum_len as f32 / count as f32;
            println!("[LOGGER] max gen: {}, mean gen: {:4.4}, mean fitness: {:1.5}, max fitness: {}, mean length: {}", max_gen, mean_gen, mean_fitness, max_fit, mean_len);
            //sleep(Duration::from_millis(1000));
        }
    });

    let analysis_period = log_freq as u64;
    let received = circbuf.clone();
    let handle = spawn(move || {
        let mut count: u64 = 0;
        for incoming in log_rx {
            let mut received = received.write().unwrap();
            received.push(incoming);
            if count % analysis_period == 0 {
                analyse_tx.send(true);
            };
            count += 1;
        }
        drop(analyse_tx);
    });

    (log_tx, handle)
}
