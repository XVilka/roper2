use crate::gen::*;
use crate::par::statics::*;
use rand::seq::IteratorRandom;
use rand::Rng;

fn mutate_arithmetic<R: Rng>(allele: &Allele, rng: &mut R) -> Allele {
    /* start basic, add more options later */
    let delta = rng.gen::<isize>() % 16;
    //println!("[+] mutate_arithmetic: delta = {}", delta);
    allele.add(delta)
}
/// One-point crossover, between two u64s, as bitvectors.
fn onept_bits<R: Rng>(a: u64, b: u64, rng: &mut R) -> u64 {
    let i = rng.gen::<u64>() % 64;
    let mut mask = ((!0) >> i) << i;
    if rng.gen::<bool>() {
        mask ^= !0
    };
    (mask & a) | (!mask & b)
}

/// Uniform crossover between two u64s, as bitvectors.
fn uniform_bits<R: Rng>(a: u64, b: u64, rng: &mut R) -> u64 {
    let mask = rng.gen::<u64>();
    (mask & a) | (!mask & b)
}

/// A simple mutation operator to use on the crossover mask,
/// prior to passing it on to the offspring.
fn random_bit_flip<R: Rng>(u: u64, rng: &mut R) -> u64 {
    if rng.gen::<f32>() < *CROSSOVER_MASK_MUT_RATE {
        u ^ (1u64 << (rng.gen::<u64>() % 64))
    } else {
        u
    }
}

fn combine_xbits<R: Rng>(m_bits: u64, p_bits: u64, combiner: MaskOp, mut rng: &mut R) -> u64 {
    match combiner {
        MaskOp::Xor => m_bits ^ p_bits,
        MaskOp::Nand => !(m_bits & p_bits),
        MaskOp::OnePt => onept_bits(m_bits, p_bits, &mut rng),
        MaskOp::Uniform => uniform_bits(m_bits, p_bits, &mut rng),
        MaskOp::And => m_bits & p_bits,
        MaskOp::Or => m_bits | p_bits,
    }
}
fn xbits_sites<R: Rng>(
    xbits: u64,
    bound: usize,
    crossover_degree: f32,
    mut rng: &mut R,
) -> Vec<usize> {
    let mut potential_sites = (0..bound)
        .filter(|x| (1u64.rotate_left(*x as u32) & xbits != 0) == *CROSSOVER_XBIT)
        .collect::<Vec<usize>>();
    potential_sites.sort();
    potential_sites.dedup();
    let num = (potential_sites.len() as f32 * crossover_degree).ceil() as usize;

    /* actual sites */
    potential_sites.into_iter().choose_multiple(&mut rng, num)
    /*
    if cfg!(debug_assertions) {
        println!("{:064b}: potential sites: {:?}", xbits, &potential_sites);
    }

    if cfg!(debug_assertions) {
        println!("actual sites: {:?}", &actual_sites);
    }
     */
}
pub fn homologous_crossover<R>(
    mother: &Creature,
    father: &Creature,
    mut rng: &mut R,
) -> Vec<Creature>
where
    R: Rng,
{
    let crossover_degree = *CROSSOVER_DEGREE;
    let bound = usize::min(mother.genome.alleles.len(), father.genome.alleles.len());
    let xbits = combine_xbits(
        mother.genome.xbits,
        father.genome.xbits,
        *CROSSOVER_MASK_COMBINER,
        rng,
    );
    let child_xbits = combine_xbits(
        mother.genome.xbits,
        father.genome.xbits,
        *CROSSOVER_MASK_INHERITANCE,
        rng,
    );
    let sites = xbits_sites(xbits, bound, crossover_degree, &mut rng);
    let mut offspring = Vec::new();
    let parents = vec![mother, father];
    let mut i = 0;
    /* Like any respectable couple, the mother and father take
     * turns inseminating one another...
     */
    while offspring.len() < 2 {
        let p0: &Creature = parents[i % 2];
        let p1: &Creature = parents[(i + 1) % 2];
        i += 1;
        let mut egg = p0.genome.alleles.clone();
        let sem = &p1.genome.alleles;
        for site in /*0..bound { // FIXME seeing if xbits help // */ sites.iter() {
            //let site = &site;
            let codon =
              /* only codons from the father are mutated, but
                since the gender of the parent is decided, each time,
                by chance, this isn't a limitation.
             */
                if rng.gen::<f32>() < *POINTWISE_MUTATION_RATE {
                    mutate_arithmetic(&sem[*site], &mut rng)
                } else {
                    sem[*site]
                };
            egg[*site] = codon;
        }
        let child_gen = usize::max(p0.genome.generation, p1.genome.generation) + 1;
        let zygote = Chain {
            alleles: egg,
            metadata: Metadata::new(),
            xbits: random_bit_flip(child_xbits, &mut rng),
            generation: child_gen,
        };
        /* The index will be filled in later, prior to filling
         * the graves of the fallen
         */
        if zygote.entry() != None {
            /* screen out the gadgetless */
            offspring.push(Creature::new(zygote, 0));
        };
        /*
          if cfg!(debug_assertions) {
              println!("WITH XBITS {:064b}, SITES: {:?}, MATED\n{}\nAND\n{}\nPRODUCING\n{}",
                       xbits,
                       &sites.iter().map(|x| x % bound).collect::<Vec<usize>>(),
                       p0, p1, &offspring[offspring.len()-1]);
              println!("************************************************************");
          }
        */
    }
    offspring
}
