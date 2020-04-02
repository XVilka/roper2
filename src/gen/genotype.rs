use rand;

use crate::emu::loader::{align_inst_addr, find_static_seg, Mode, Seg, MEM_IMAGE};
use crate::par::statics::*;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;

use self::rand::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gadget {
    pub ret_addr: u64,
    pub entry: u64,
    pub sp_delta: usize,
    pub mode: Mode,
}

impl Gadget {
    fn add(self, other: i64) -> Gadget {
        let seg = find_static_seg(self.entry);
        match seg {
            /* Guard against overflow! FIXME */
            Some(seg) => {
                //println!("[+] Seg: {}", seg);
                let offset = self.entry as i64 - seg.addr as i64;
                let new_offset = (offset + other) % seg.memsz as i64;
                //println!("[+] seg.addr = 0x{:x}, offset = 0x{:x}, new_offset = 0x{:x}", seg.addr, offset, new_offset);
                let new_entry = (seg.addr as i64 + new_offset) as u64;
                //println!("[+] Adding {} to gadget with entry 0x{:x} to create gadget with entry 0x{:x}", other, self.entry, new_entry);
                Gadget {
                    ret_addr: self.ret_addr, /* TODO: Update ret_addr with analysis */
                    entry: new_entry,
                    sp_delta: self.sp_delta, /* TODO: Update with analysis */
                    mode: self.mode,         /* TODO: update if in ARM and other is odd */
                }
            }
            None => {
                println!("[x] Couldn't find segment for address 0x{:x}!", self.entry);
                self
            }
        }
    }
}
//unsafe impl Send for Gadget {}

pub const ENDIAN: Endian = Endian::Little;

impl Display for Gadget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[Entry: {}, Ret: {}, SpD: {:x}, Mode: {:?}]",
            wf(self.entry),
            wf(self.ret_addr),
            self.sp_delta,
            self.mode
        )
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Endian {
    Big,
    Little,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Allele {
    //Const(u64),
    Input(usize),
    Gadget(Gadget),
}

impl Allele {
    pub fn entry(&self) -> Option<u64> {
        match *self {
            Allele::Gadget(g) => Some(g.entry),
            _ => None,
        }
    }

    pub fn add(&self, addend: isize) -> Self {
        match *self {
            /* FIXME: Assuming limit of 256 input slots, but hardcoded... */
            Allele::Input(n) => Allele::Input(((n as isize + addend) % 256) as usize),
            Allele::Gadget(n) => Allele::Gadget(n.add(addend as i64)),
        }
    }
}
//unsafe impl Send for Allele {}

impl Display for Allele {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            //&Allele::Const(x) => write!(f, "[Const {}]", wf(x)),
            Allele::Input(i) => write!(f, "[Input Slot #{}]", i),
            Allele::Gadget(g) => write!(f, "{}", g),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chain {
    pub alleles: Vec<Allele>,
    pub metadata: Metadata,
    pub xbits: u64, /* used to coordinate crossover and speciation */
    pub generation: usize,
}

impl Display for Chain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        //let mut s = Vec::new();
        //let mut pad_offset = 0;
        for allele in self.alleles.iter() {
            writeln!(f, "\t{}", allele)?;
        }
        writeln!(f, "\tXBITS: {:064b}", self.xbits)?;
        writeln!(f, "\tGEN: {}", self.generation)
    }
}

impl Chain {
    pub fn len(&self) -> usize {
        self.alleles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.alleles.is_empty()
    }

    pub fn pack(&self, input: &[u64]) -> Vec<u8> {
        let mut p: Vec<u8> = Vec::new();
        /*
        let mut pad_offset = 0;
        for gad in self.gads.iter() {
            let mut w = gad.entry;
            /* Jumps to thumb addresses are indicated by a LSB of 1 */
            /* NB: Check to make sure Unicorn is actually following this */
            if gad.mode == Mode::Thumb { w |= 1 };
            let wp = pack_word(w, *ADDR_WIDTH, ENDIAN);
            p.extend_from_slice(&wp);
            /* now, pack as many pads as needed to saturate sp_delta */
            if gad.sp_delta <= 1 { continue };
            let padnum = self.pads.len();
            if padnum == 0 { continue };
            for i in 0..(gad.sp_delta-1) {
                let o = i + pad_offset;
                let w = match self.pads[o % padnum] {
                    Allele::Const(x) => x,
                    Allele::Input(i) => if input.len() > 0 {
                        input[o % input.len()]
                    } else { 0 },
                };
                let wp = pack_word(w, *ADDR_WIDTH, ENDIAN);
                p.extend_from_slice(&wp);
            }
            pad_offset += gad.sp_delta-1;
        }
        */
        let mut start = false;
        for allele in self.alleles.iter() {
            if allele.entry() == None && !start {
                continue;
            } else {
                start = true;
            };
            let w = match *allele {
                //Allele::Const(c) => c,
                Allele::Input(i) => {
                    if !input.is_empty() {
                        input[i % input.len()]
                    } else {
                        0
                    }
                }
                Allele::Gadget(g) => g.entry,
            };
            p.extend_from_slice(&pack_word(w, *ADDR_WIDTH, ENDIAN));
        }
        p
    }

    pub fn entry(&self) -> Option<u64> {
        for allele in self.alleles.iter() {
            if let Some(e) = allele.entry() {
                return Some(e);
            };
        }
        println!("WARNING! NO ENTRY! NO GADGETS IN CHAIN?");
        println!("{}", self);
        None
    }

    /* TODO: create a separate thread that maintains the
     * pool of random seeds, and serves them on request,
     * over a channel, maybe.
     */
    /* TODO alignment function, which depends on ARCHITECTURE */
    pub fn from_seed<R>(rng: &mut R, len_range: (usize, usize)) -> Self
    where
        R: Rng,
    {
        let xbits: u64 = rng.gen::<u64>();

        let input_slot_freq = INPUT_SLOT_FREQ;
        let exec_segs = MEM_IMAGE
            .iter()
            .filter(|s| s.is_executable())
            .collect::<Vec<&Seg>>();

        let mut alleles: Vec<Allele> = Vec::new();
        let (min_len, max_len) = len_range;
        let range = usize::max(1, max_len - min_len);
        let glen = rng.gen::<usize>() % range + min_len;

        for _ in 0..glen {
            let seg = &exec_segs[rng.gen::<usize>() % exec_segs.len()];
            let unaligned_addr = seg.aligned_start() + rng.gen::<u64>() % seg.aligned_size() as u64;
            let mode = ARCHITECTURE.mode(); /* choose mode randomly if ARM */
            let addr = align_inst_addr(unaligned_addr, mode);
            /* sp_delta-informed chance of choosing const or input TODO */
            if !alleles.is_empty() && rng.gen::<f32>() < input_slot_freq {
                /* NOTE: Artificially adding an upper bound on the number of inputs
                 * at 15. This will almost certainly be more than enough, and will
                 * make the input slots easier to read.
                 */
                alleles.push(Allele::Input(rng.gen::<usize>() & 0x0F));
            } else {
                let gad = Gadget {
                    entry: addr,
                    ret_addr: 0, /* TODO */
                    sp_delta: 0, /* TODO */
                    mode,        /* TODO - for ARM decide mode */
                };

                alleles.push(Allele::Gadget(gad));
            }
        }

        /*
        let pad_num = gads.iter()
                          .map(|x| x.sp_delta)
                          .sum::<usize>();
        let mut pads = Vec::new();
        for _ in 0..pad_num {
            let pad = if rng.gen::<f32>() < input_slot_freq {
                Allele::Input(rng.gen::<usize>())
            } else {
                Allele::Const(rng.gen::<u64>())  /* TODO take const range param? */
            };
            pads.push(pad);
        }
        */

        Chain {
            alleles,
            xbits,
            metadata: Metadata::new(),
            generation: 0,
        }
    }
}

/* by using a hashmap instead of separate struct fields
 * for the various bits of metadata, we end up with a
 * much more flexible structure, that won't require
 * dozens of fiddly signature changes every time we
 * want to add or modify a field. f32 should work
 * for most of the fields we're interested in.
 * We can dispense with Option fields, by just letting
 * "None" be the absence of a field in the hashmap.
 * Accessor functions will provide an easy interface.
 */

#[derive(Default, Clone, Debug, PartialEq)]
pub struct Metadata(pub HashMap<&'static str, f32>);
impl Metadata {
    pub fn new() -> Self {
        Metadata(HashMap::new())
    }
}

fn pack_word(word: u64, size: usize, endian: Endian) -> Vec<u8> {
    let mut p = match size {
        4 => {
            let w32 = if endian == Endian::Big {
                (word & 0xFFFFFFFF00000000) as u32
            } else {
                (word & 0x00000000FFFFFFFF) as u32
            };
            pack_word32le(w32)
        }
        8 => pack_word64le(word),
        _ => panic!("Bad word size. Must be either 4 or 8."),
    };
    if endian == Endian::Big {
        p.reverse()
    };
    p
}

pub fn pack_word32le(word: u32) -> Vec<u8> {
    let mut p: Vec<u8> = Vec::new();
    p.extend_from_slice(&[
        (word & 0xFF) as u8,
        ((word & 0xFF00) >> 0x08) as u8,
        ((word & 0xFF0000) >> 0x10) as u8,
        ((word & 0xFF000000) >> 0x18) as u8,
    ]);
    p
}

pub fn pack_word32le_vec(v: &[u32]) -> Vec<u8> {
    let mut p: Vec<u8> = Vec::new();
    for word in v {
        p.extend_from_slice(&pack_word32le(*word))
    }
    p
}

pub fn pack_word64le(word: u64) -> Vec<u8> {
    let (hi, lo) = (
        ((word & 0xFFFFFFFF00000000) >> 0x20) as u32,
        (word & 0xFFFFFFFF) as u32,
    );
    let mut p = pack_word32le(lo);
    p.extend_from_slice(&pack_word32le(hi));
    p
}

pub fn pack_word64le_vec(v: &[u64]) -> Vec<u8> {
    let mut p: Vec<u8> = Vec::new();
    for word in v {
        p.extend_from_slice(&pack_word64le(*word));
    }
    p
}
