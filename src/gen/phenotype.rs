extern crate evmap;

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::sync::{Arc, Mutex};
use std::hash::{Hash, Hasher};
use std::fmt;
use std::fmt::Display;

use rand::{Rng, SeedableRng};
use rand_isaac::isaac64::Isaac64Rng;

use crate::genotype::*;
use crate::emu::loader::Mode;
use crate::par::statics::*;
use crate::log;

#[derive(   Clone, Debug, PartialEq, Eq)]
pub struct WriteRecord {
    pub pc: u64,
    pub dest_addr: u64,
    pub value: u64,
    pub size: usize,
}


pub fn collapse_writelog(writelog: &Vec<WriteRecord>) -> Vec<WriteRecord> {
    /* create order preserving set (?), keyed to address
    for each record, in order of execution, clobber any existing
    record that writes to the same address */
    /* each item is a pair: order of execution, write record */
    let mut record: HashMap<u64, (usize, WriteRecord)> = HashMap::new();
    let mut order_of_exec = 0;
    for wr in writelog.iter() {
        record.insert(wr.dest_addr, (order_of_exec, wr.clone()));
        order_of_exec += 1;
    }
    let mut collapsed = record.values().collect::<Vec<&(usize, WriteRecord)>>();
    collapsed.sort_by_key(|(ord,_)| ord);
    let mut result = Vec::new();
    for (_,w) in collapsed {
        result.push(w.clone())
    }
    result
}


#[derive(   Clone, Debug, PartialEq, Eq)]
pub struct VisitRecord {
    pub pc: u64,
    pub mode: Mode,
    pub inst_size: usize,
    pub registers: Vec<u64>,
}

impl Display for VisitRecord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}    [REGS: {}]",
            log::disas_static(self.pc, self.inst_size, self.mode, 1),
            self.registers
                .iter()
                .map(|r| format!("{:x}", r))
                .collect::<Vec<String>>()
                .join(" ")
        )
    }
}

#[derive(   Clone, Debug, PartialEq)]
pub struct Pod {
    pub registers: Vec<u64>,
    pub visited: Vec<VisitRecord>,
    pub writelog: Vec<WriteRecord>,
    pub retlog: Vec<u64>,
}

impl Pod {
    pub fn new(
        registers: Vec<u64>,
        visited: Vec<VisitRecord>,
        writelog: Vec<WriteRecord>,
        retlog: Vec<u64>,
    ) -> Self {
        Pod {
            registers: registers,
            visited: visited,
            writelog: collapse_writelog(&writelog),
            retlog: retlog,
        }
    }
    
    pub fn retlog(&self) -> &Vec<u64> {
        &self.retlog
    }

    pub fn retlog_len(&self) -> usize {
        self.retlog.len()
    }

    pub fn collapse_writelog(&self) -> Vec<WriteRecord> {
        /* create order preserving set (?), keyed to address
        for each record, in order of execution, clobber any existing
        record that writes to the same address */
        /* each item is a pair: order of execution, write record */
        let mut record: HashMap<u64, (usize, WriteRecord)> = HashMap::new();
        let mut order_of_exec = 0;
        for wr in self.writelog.iter() {
            record.insert(wr.dest_addr, (order_of_exec, wr.clone()));
            order_of_exec += 1;
        }
        let mut collapsed = record.values().collect::<Vec<&(usize, WriteRecord)>>();
        collapsed.sort_by_key(|(ord,_)| ord);
        let mut result = Vec::new();
        for (_,w) in collapsed {
            result.push(w.clone())
        }
        result
    }

    pub fn writelog_len(&self) -> usize {
        self.writelog.len()
    }
    /// Dump a vector of strings containing the disassembly
    /// of each address visited by the phenotype.
    pub fn disas_visited(&self) -> Vec<String> {
        let mut v = Vec::new();
        for vrec in &self.visited {
            v.push(format!("{}", vrec));
        }
        v
    }

    /// Dump information about the writes performed by the
    /// phenotype.
    /// TODO: adjust the word size used in these contexts, dependent on
    /// architecture. (FIXME)
    pub fn dump_written(&self) -> Vec<String> {
        let mut v = Vec::new();
        for wrec in &self.writelog {
            let row = format!(
                "{}: {} -> {} | {}",
                wf(wrec.pc),
                wf(wrec.dest_addr),
                wf(wrec.value),
                log::disas(
                    &pack_word64le(wrec.value)[0..wrec.size].to_vec(),
                    ARCHITECTURE.mode(),
                    wrec.size
                )
            ); /* up to 1 inst per byte */
            v.push(row);
        }
        v
    }

    /* FITNESS FUNCTIONS (PODWISE) */
    /* let's start with something simple: we'll reward the number of
     * unique returns
     */

    fn ff_uniq_retcount(&self) -> usize {
        let mut rl = self.retlog.clone();
        rl.sort();
        rl.dedup();
        //let upper_bound = 128;
        //let retscore = usize::min(rl.len(), upper_bound);
        //retscore as f32 / upper_bound as f32
        //if rl.len() == 0 { 1.0 } else { 1.0 / rl.len() as f32 }
        rl.len() /* setting 0 as least fitness rather than 1.0
         * may turn out to be less restrictive */
    }

}
    //unsafe impl Send for Pod {}
    
    /* Retain the Pod after hatching. Initialized genomes in an otherwise
 * empty Pod. Or with an Option<Pod>. We only ever need to hatch a
 * genome /once/ -- even with fitness sharing, we can just re-evaluate
 * the hatched phenome with different parameters. But that part of the
 * evaluation is a one-shot deal. ROPER I made the mistake of tightly
 * coupling the hatching procedure with the "eval_case" procedure. This
 * doesn't need to be done that way.
 */

pub type Input = Vec<u64>; /* a static reference would be better FIXME */
pub type Phenome = HashMap<Input, Option<Pod>>;
pub type Fitness = Vec<f32>;

pub trait FitnessOps {
    fn mean(&self) -> f32;
}

impl FitnessOps for Fitness {
    fn mean(&self) -> f32{
       self.iter().sum::<f32>() / self.len() as f32
    }
}

pub trait FitFuncs {
    fn avg_retlog_len(&self) -> usize;
    fn mean_podwise_fitness<F>(&self, ff: F) -> f32 where F: FnMut(&Pod) -> usize;
    fn ff_mean_uniq_retcount(&self) -> f32;
    fn ff_mean_retcount(&self) -> f32;
    fn ff_mean_writecount(&self) -> f32;
}

impl FitFuncs for Phenome {

    fn avg_retlog_len(&self) -> usize {
        let mut sum = 0;
        let mut count = 0;
        for (_input, op_pod) in self.iter() {
            match op_pod {
                Some(pod) => {
                    count += 1;
                    sum += pod.retlog.len();
                },
                None => (),
            }
        }
        if count == 0 {
            0
        } else {
            sum / count
        }
    }

    fn mean_podwise_fitness<F>(&self, ff: F) -> f32
    where
        F: FnMut(&Pod) -> usize,
    {
        let scores = self.values()
            .filter(| &pod | *pod != None)
            .map(| ref pod | pod.as_ref().unwrap())
            .map(ff)
            .collect::<Vec<usize>>();
        //println!("[mean_podwise_fitness] {:?}", scores);
        if scores.len() == 0 {
            0.0
        } else {
            scores.iter().sum::<usize>() as f32 / scores.len() as f32
        }
    }

    fn ff_mean_uniq_retcount(&self) -> f32 {
        self.mean_podwise_fitness(Pod::ff_uniq_retcount)
    }

    fn ff_mean_retcount(&self) -> f32 {
        self.mean_podwise_fitness(Pod::retlog_len)
    }

    fn ff_mean_writecount(&self) -> f32 {
        self.mean_podwise_fitness(Pod::writelog_len)
    }

 
}

pub trait Pareto {
    fn dominated_by(&self, other: &Fitness) -> bool;
}
/* Implement some sort of Pareto comparison here */
/* we'll probably want to move this into its own module, eventually. */
impl Pareto for Fitness {
    fn dominated_by(&self, other: &Fitness) -> bool {
        let mut dom = true;
        for (x,y) in self.iter().zip(other.iter()) {
            if x > y {
                dom = false;
                break
            }
        }
        dom
    }
}

#[derive(   Debug, Clone)]
pub struct Creature {
    pub genome: Chain,
    pub phenome: Phenome,
    pub index: usize,
    pub metadata: Metadata,
    pub name: String,
    pub fitness: Option<Fitness>,
}

impl PartialEq for Creature {
    fn eq(&self, other: &Creature) -> bool {
        self.name == other.name
    }
}

impl Eq for Creature {}

impl Display for Creature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "BIOGRAPHY OF {}\nGENOME:\n{}\nPHENOME:\n{}\n{}\n{:?}",
            self.name,
            self.genome,
            self.disas_visited().join("\t\n"),
            self.dump_written().join("\t\n"),
            self.fitness
        )
    }
}

fn baptise_chain(chain: &Chain) -> String {
    let syllables = 8;
    let p = chain.pack(&Vec::new());
    let mut hasher = DefaultHasher::new();
    p.hash(&mut hasher);
    let hash: u64 = hasher.finish();
    /* now, convert that hash to a pronounceable name */
    let consonants = vec![
        'b', 'c', 'd', 'f', 'g', 'h', 'j', 'k', 'l', 'm', 'n', 'v', 'w', 'x', 'z', 'y'
    ];
    let vowels = vec!['a', 'e', 'i', 'o', 'u'];
    let hbytes = pack_word64le(hash);
    let mut letters = Vec::new();
    assert!(syllables <= hbytes.len());
    for i in 0..syllables {
        letters.push(consonants[(hbytes[i] as usize) % consonants.len()]);
        letters.push(vowels[(hbytes[i] as usize) % vowels.len()]);
        letters.push(consonants[(hbytes[i] as usize) % consonants.len()]);
        if i % 2 == 1 && i < syllables - 1 {
            letters.push('-')
        };
    }
    letters.iter().collect::<String>()
}

impl Creature {
    pub fn new(genome: Chain, index: usize) -> Self {
        let name = baptise_chain(&genome);
        Creature {
            genome: genome,
            phenome: Phenome::new(),
            index: index,
            metadata: Metadata::new(),
            name: name,
            fitness: None,
        }
    }

    pub fn inherit_problems(&mut self, parent: &Creature) -> () {
        let mut larval_phenome = Phenome::new();
        let mut has_input = false;
        for (input, _) in parent.phenome.iter() {
            larval_phenome.insert(input.clone(), None);
            has_input = true;
        }
        assert!(has_input);
        self.phenome = larval_phenome;
    }

    pub fn ab_fit(&self) -> Option<f32> {
        match self.metadata.0.get("ab_fit") {
            None => None,
            Some(&x) => Some(x),
        }
    }

    pub fn set_ab_fit(&mut self, ab_fit: f32) -> () {
        self.metadata.0.insert("ab_fit", ab_fit);
    }

    pub fn pose_problem(&mut self, input: &Input) -> () {
        self.phenome.insert(input.clone(), None);
    }

    pub fn disas_visited(&self) -> Vec<String> {
        let mut dump = Vec::new();
        for (input, pod) in &self.phenome {
            if pod == &None {
                continue;
            };
            dump.push(format!(
                "ON INPUT {:?}, VISITED:\n\t{}\nRETS: {}",
                input,
                pod.as_ref().unwrap().disas_visited().join("\n\t"),
                pod.as_ref()
                    .unwrap()
                    .retlog
                    .iter()
                    .map(|x| wf(*x))
                    .collect::<Vec<String>>()
                    .join(" ")
            ));
        }
        dump
    }

    pub fn dump_written(&self) -> Vec<String> {
        let mut dump = Vec::new();
        for (input, pod) in &self.phenome {
            if pod == &None {
                continue;
            };
            dump.push(format!(
                "ON INPUT {:?}, WROTE:\n\t{}",
                input,
                pod.as_ref().unwrap().dump_written().join("\n\t")
            ));
        }
        dump
    }

    /* returns true if the Creature has hatched -- if its
     * phenotype has developed -- and false otherwise.
     */
    pub fn has_hatched(&self) -> bool {
        0 < self.phenome.iter().filter(|(_,v)| v != &&None).count()
    }

    pub fn generation(&self) -> usize {
        self.genome.generation
    }
}

//unsafe impl Send for Creature {}

type Larva = Mutex<Creature>;

fn larvalise(creature: Creature) -> Larva {
    Mutex::new(creature)
}

/* ok, evmap won't work. */
#[derive(Debug, Clone)]
pub struct Population {
    pub hive: Arc<Vec<Mutex<Arc<RefCell<Creature>>>>>,
}

impl Population {
    /// What we want to do here is to create an indexable
    /// collection of creatures that can be individually
    /// and mutably accessed by arbitrary threads.
    pub fn new(creatures: Vec<Creature>) -> Self {
        let mut mutexed_creatures = Vec::new();
        let mut creatures = creatures;
        while creatures.len() > 0 {
            mutexed_creatures.push(Mutex::new(Arc::new(RefCell::new(creatures.pop().unwrap()))))
        }

        Population {
            hive: Arc::new(mutexed_creatures),
        }
    }

    /// Selects num individuals at random, using the RngSeed, and
    /// ensures that the chosen are all internally mutable, wrt their
    /// RwLocks.
    pub fn choose(&self, seed: RngSeed, num: usize) -> Vec<Arc<RefCell<Creature>>> {
        let mut rng = Isaac64Rng::from_seed(seed);
        //sample(&mut rng, self.hive, num)
        // choose unlocked
        let mut chosen = Vec::new();
        /* Careful here. Could this cause hold-and-wait deadlocks?
         * Maybe have a mutex on the hive to guard against that.
         */
        while chosen.len() < num {
            let i = rng.gen::<usize>() % self.hive.len();
            match self.hive[i].try_lock() {
                Err(_) => continue,
                Ok(x) => chosen.push(x.clone()),
            }
            /* if you observe a lot of busy waiting here, try adding a very
             * short sleep() (especially in small populations)  */
        }
        chosen
    }
}
