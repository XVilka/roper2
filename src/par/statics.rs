extern crate goblin;
extern crate num;
extern crate rand;
extern crate ini;

use std::fs::File;
use std::sync::{Arc, RwLock};
use std::io::Read;
use std::path::Path;
use std::env;
use std::fmt;

use self::ini::Ini;
use self::num::PrimInt;
use self::goblin::Object;
use self::goblin::elf::header::machine_to_str;

use crate::emu::loader::{Arch, Mode};
    lazy_static! {
        pub static ref ROPER_INI_PATH: String
            = match env::var("ROPER_INI_PATH") {
                    Err(_) => ".roper_config/roper.ini".to_string(),
                    Ok(d)  => d.to_string(),
              };
    }
lazy_static! {
    pub static ref INI: Ini = Ini::load_from_file(&*ROPER_INI_PATH)
        .expect(&format!("Failed to load init file from {}", &*ROPER_INI_PATH));
}
pub type RngSeed = [u8; 32];

lazy_static! {
    pub static ref RNG_SEED: RngSeed /* for Isaac64Rng */
        = {
            let rand_sec = INI.section(Some("Random".to_owned()))
                .expect("couldn't find [Random] section in ini file");
            let seed_txt = rand_sec.get("seed")
                .expect("couldn't get seed field from [Random] section");
            println!("[RNG_SEED] {}", seed_txt);
            let mut seed_vec = [0u8; 32];
            let mut i = 0;
            for octet in seed_txt.split_whitespace() {
                seed_vec[i] = u8::from_str_radix(octet,16).expect("Failed to parse seed");
                i += 1;
                if i == 32 { break };
            }
            println!("[RNG_SEED] {:?}", seed_vec);
            seed_vec
        };
}
lazy_static! {
    pub static ref CODE_BUFFER: Vec<u8>
        = {
            /* first, read the config */
            let bp = match env::var("ROPER_BINARY") {
                Ok(s)  => s.to_string(),
                Err(_) => {
                    INI.section(Some("Binary"))
                        .expect("Couldn't find Binary section in INI")
                        .get("path")
                        .expect("Couldn't find path field in Binary section of INI")
                        .to_string()
                }
            };
            //println!("[*] Read binary path as {:?}",bp);
            let path = Path::new(&bp);
            let mut fd = File::open(path).expect(&format!("Can't read binary at {:?}",bp));
            let mut buffer = Vec::new();
            fd.read_to_end(&mut buffer).unwrap();
            buffer
        };
}


// set addr size here too. dispense with risc_width() calls, which are confused
lazy_static! {
    pub static ref ARCHITECTURE: Arch
        = {
            let arch_magic = match Object::parse(&CODE_BUFFER).unwrap() {
                Object::Elf(e) => machine_to_str(e.header.e_machine),
                _ => panic!("Binary format unimplemented."),
            };
            match arch_magic {
                "ARM" => Arch::Arm(Mode::Arm),
                "MIPS"  => Arch::Mips(Mode::Be),
                "MIPS_RS3_LE" => Arch::Mips(Mode::Le),
                "X86_64" => Arch::X86(Mode::Bits64),
                "386" => Arch::X86(Mode::Bits32),
                _  => panic!("arch_magic not recognized!"),
            }
        };
}

lazy_static! {
    pub static ref ADDR_WIDTH: usize
        = {
            match *ARCHITECTURE {
                Arch::X86(_) => 8,
                _ => 4,
            }
        };
}

/// A tiny machine word formatter
#[inline]
pub fn wf<T: PrimInt + fmt::LowerHex>(w: T) -> String {
    match *ARCHITECTURE {
        Arch::X86(Mode::Bits64) => format!("{:016x}", w),
        Arch::X86(Mode::Bits16) => format!("{:04x}", w),
        _ => format!("{:08x}", w),
    }
}

lazy_static! {
    pub static ref KILL_SWITCH: Arc<RwLock<bool>>
        = Arc::new(RwLock::new(false));
}

pub const INPUT_SLOT_FREQ: f32 = 0.1;

lazy_static! {
    pub static ref CROSSOVER_DEGREE: f32 = 0.5;
    /* TODO: Read this from a config file */
}

fn lookup_usize_setting (section: &str, item: &str, default: usize) -> usize {
    let default = format!("{}", default); /* KLUDGE */ 
    let str_setting = lookup_string_setting(section, item, default);
    (&str_setting).parse::<usize>().unwrap()
}

fn lookup_string_setting (section: &str, item: &str, default: String) -> String {
    let sec = INI.section(Some(section.to_owned()));
    match sec {
        None => default,
        Some(s) => s.get(item).unwrap_or(&default).to_string()
    }
}

fn lookup_f32_setting (section: &str, item: &str, default: f32) -> f32 {
    let sec = INI.section(Some(section.to_owned()));
    let dstr = format!("{}",default); /* KLUDGE */
    match sec {
        None => default,
        Some(s) => s.get(item).unwrap_or(&dstr)
            .parse::<f32>()
            .unwrap(),
    }
}



lazy_static! {
    pub static ref TSIZE: usize = 
        lookup_usize_setting ("Selection", "tournament_size", 32);
}

lazy_static! {
    pub static ref MATE_SELECTION_FACTOR: f32 =
        lookup_f32_setting ("Selection", "mate_selection_factor", 1.00);
}



lazy_static! {
    /* if true, then homologous xbit crossover selects only those slots
     * for which mbit ^ pbit == 1. if false, it selects only those slots
     * for which mbit ^ pbit == 0.
     */
    pub static ref CROSSOVER_XBIT: bool = true;
    /* TODO read from config file */
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MaskOp {
    Xor,
    Nand,
    OnePt,
    Uniform,
    And,
    Or,
}

lazy_static! {
    /* TODO read from config file */
    pub static ref CROSSOVER_MASK_COMBINER: MaskOp = MaskOp::Xor;
    /* I may have stumbled upon something interesting here. using Xor masks
    on the xbits appears to forestall premature convergence! Which makes sense, 
    if you think about it -- it forces crossover to at most periodically cycle
    around a fixed genotype, giving the gene pool a bit more room to breathe. */
}

lazy_static! {
    /* TODO read */
    pub static ref CROSSOVER_MASK_INHERITANCE: MaskOp = MaskOp::Uniform;
} 

lazy_static! {
    /* TODO read */
    pub static ref CROSSOVER_MASK_MUT_RATE: f32 = 0.2;
}

lazy_static! {
    pub static ref POINTWISE_MUTATION_RATE: f32 =
        lookup_f32_setting ("Mutation", "pointwise_mutation_rate", 0.01);
}

lazy_static! {
    pub static ref CHANNEL_SIZE: usize =
        lookup_usize_setting ("Concurrency", "channel_size", 1);
}

lazy_static! {
    pub static ref SELECTION_WINDOW_SIZE: usize =
        lookup_usize_setting ("Selection", "selection_window_size", 15);
}

lazy_static! {
    pub static ref POPULATION_SIZE: usize =
        lookup_usize_setting ("Population", "population_size", 0x1000);
}

lazy_static! {
    pub static ref MIN_CREATURE_LENGTH: usize =
        lookup_usize_setting ("Population", "min_creature_length", 2);
}

lazy_static! {
    pub static ref MAX_CREATURE_LENGTH: usize =
        lookup_usize_setting ("Population", "max_creature_length", 2);
}



lazy_static! {
    pub static ref NUM_ENGINES: usize =
        lookup_usize_setting("Concurrency", "num_engines", 16);
}


lazy_static! {
    pub static ref LOG_DIRECTORY: String =
        lookup_string_setting("Logging", "log_directory", "./logs".to_string());
}
