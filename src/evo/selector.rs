pub fn spawn_breeder(
        window_size: usize,
        rng_seed: RngSeed,
    ) -> (Sender<Creature>, Receiver<Creature>, JoinHandle<()>) {
        let (from_breeder_tx, from_breeder_rx) = channel();
        let (into_breeder_tx, into_breeder_rx) = channel();

        let window = Arc::new(RefCell::new(Vec::with_capacity(window_size+1)));
        let mut rng_seed = rng_seed.clone();

        let sel_handle = spawn(move || {
            let window = window.clone();
            for creature in into_breeder_rx {
                let mut window = window.borrow_mut();
                window.push(creature);
                if window.len() >= window_size {
                    /* then it's time to select breeders */
                    rng_seed = tournament(&mut window, rng_seed);
                    /* now send them back. new children will have replaced the dead */
                    for creature in window {
                        from_breeder_rx.send(creature)
                    }
                    /* Now, flush the window out so that it can refill. */
                    window.truncate(0);
                }
            }
        });

        (into_breeder_tx, from_breeder_rx, sel_handle)
    }
fn tournament(selection_window: &mut Vec<Creature>,
              seed: RngSeed) -> RngSeed {
    let mut rng = Isaac64Rng::from_seed(RngSeed);
    /* note: seed creation should probably be its own utility function */
    let mut new_seed: [u8; 32] = [0; 32];
    for i in 0..32 { new_seed[i] = rng.gen::<u8>() }

    assert!(*TSIZE * *MATE_SELECTION_FACTOR <= selection_window.len());
    let indices = rand::seq::sample_indices(&mut rng,
                                            selection_window.len(),
                                            *TSIZE * *MATE_SELECTION_FACTOR);
    /* TODO: take n times as many combatants as needed, then winnow
     * out those least compatible with first combatant's crossover mask
     */
    let compatkey = |i| {
        64 - xover_compat(&selection_window[0], &selection_window[i])
    }
    indices.sort_by_key(compatkey);
    /* now drop the least compatible from consideration */
    indices.truncate(*TSIZE);

    let fitkey = |i| {
        &selection_window[i].fitness
    }
    /* now, sort the remaining indices by the fitness of their creatures */
    /* TODO -- we need a pareto sorting function */
    indices.sort_by_key(fitkey);
    /* and choose the parents and the fallen */
    // *TSIZE must be >= 4.
    let (p0, p1) = (indices[*TSIZE-1], indices[*TSIZE-2]);
    let (d0, d1) = (indices[0], indices[1]);
    let (mother, father) = (&selection_window[p0], &selection_window[p1]);
    let offspring = homologous_crossover(&mother, &father, &mut rng);
    selection_window[d0] = offspring[0];
    selection_window[d1] = offspring[1];

    /* the tournament function calls the mating functions, replacing the fallen 
     * with the offspring of the victors 
     */
    new_seed
}
