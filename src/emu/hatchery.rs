// [[file:~/src/roper2/src/emu/hatchery.org::hatch][hatch]]
use std::thread::{spawn, JoinHandle};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::rc::Rc;
use std::cell::RefCell;
use crate::emu::loader::{get_mode, read_pc, uc_general_registers, Engine};
use crate::par::statics::*;
use crate::gen;
use crate::gen::phenotype::{VisitRecord, WriteRecord};
/* An expect of 0 will cause this loop to run indefinitely */
pub fn spawn_hatchery(
    num_engines: usize,
) -> (
    SyncSender<gen::Creature>,
    Receiver<gen::Creature>,
    JoinHandle<()>,
) {

    let (from_hatch_tx, from_hatch_rx)
        : (SyncSender<gen::Creature>, Receiver<gen::Creature>)
        = sync_channel(*CHANNEL_SIZE);
    let (into_hatch_tx, into_hatch_rx)
        : (SyncSender<gen::Creature>, Receiver<gen::Creature>)
        = sync_channel(*CHANNEL_SIZE);

    let handle = spawn(move || {
        let mut carousel = Vec::new();

        for _ in 0..num_engines {
            let (eve_tx, eve_rx) = sync_channel(*CHANNEL_SIZE);
            let from_hatch_tx = from_hatch_tx.clone();
            let h = spawn(move || {
                spawn_coop(eve_rx, from_hatch_tx);
            });
            carousel.push((eve_tx, h));
        }

        let mut coop = 0;
        let mut counter = 0;
        let already_hatched_tx = from_hatch_tx.clone();
        let mut num_already_hatched = 0;
        for incoming in into_hatch_rx {
            let &(ref tx, _) = &carousel[coop];
            let carousel_tx = tx.clone();
            /* So long as the phenotype of a Creature is uniquely determineed
             * by its genotype, we can just skip over those creatures that
             * have already been hatched, returning them. But this might have
             * the unfortunate consequence that old Creatures crowd the head
             * of the channel. We'll see how serious an issue this is when we
             * come to it.
             */
            if incoming.has_hatched() {
                num_already_hatched += 1;
                already_hatched_tx.send(incoming).unwrap();
            } else {
                carousel_tx.send(incoming).unwrap();
                counter += 1;
            }
            coop = (coop + 1) % carousel.len();
            if (counter + num_already_hatched) % 100000 == 0 {
              println!("[{} Emulations; num_already_hatched = {}; ratio new: {}]",
                       counter, num_already_hatched, (counter as f32 / (num_already_hatched + counter) as f32));
            }
            drop(tx);
        }
        /* clean up the carousel */
        while carousel.len() > 0 {
            if let Some((tx, h)) = carousel.pop() {
              println!(")-- cleaning up {:?} --(", tx);
                drop(tx);
                h.join().unwrap();
            };
        }
    });

    (into_hatch_tx, from_hatch_rx, handle)
}
fn spawn_coop(rx: Receiver<gen::Creature>,
              tx: SyncSender<gen::Creature>) {
    /* a thread-local emulator */
    let mut emu = Engine::new(*ARCHITECTURE);

    /* Hatch each incoming creature as it arrives, and send the creature
     * back to the caller of spawn_hatchery. */
    for incoming in rx {
        let mut creature = incoming;
        let phenome = hatch_cases(&mut creature, &mut emu);
        creature.phenome = phenome;
        if !creature.has_hatched() {
            println!("[in spawn_coop] This bastard hasn't hatched!\n{}", creature);
            std::process::exit(1);
        }
        tx.send(creature).unwrap(); /* goes back to the thread that called spawn_hatchery */
    }
}
#[inline]
pub fn hatch_cases(creature: &mut gen::Creature, emu: &mut Engine)
                   -> gen::Phenome {
    let mut map = gen::Phenome::new();
    {
        let mut inputs: Vec<gen::Input> =
            creature.phenome.keys().cloned().collect();
        assert!(!inputs.is_empty());
        while !inputs.is_empty() {
            let input = inputs.pop().unwrap();
            /* This can't really be threaded, due to the unsendability of emu */
            let pod = hatch(creature, &input, emu);
            map.insert(input.to_vec(), Some(pod));
        }
    }
    map
}
  #[inline]
  pub fn hatch(creature: &mut gen::Creature,
               input: &gen::Input,
               emu: &mut Engine) -> gen::Pod {
      let mut payload = creature.genome.pack(input);
      let start_addr = creature.genome.entry().unwrap();
      /* A missing entry point should be considered an error,
       * since we try to guard against this in our generation
       * functions.
       */
      let (stack_addr, stack_size) = emu.find_stack();
      payload.truncate(stack_size / 2);
      let _payload_len = payload.len();
      let stack_entry = stack_addr + (stack_size / 2) as u64;
      emu.restore_state().unwrap();

      /* load payload **/
      emu.mem_write(stack_entry, &payload)
          .expect("mem_write fail in hatch");
      emu.set_sp(stack_entry + *ADDR_WIDTH as u64).unwrap();

    let visitor: Rc<RefCell<Vec<VisitRecord>>> = Rc::new(RefCell::new(Vec::new()));
    let writelog = Rc::new(RefCell::new(Vec::new()));
    let retlog = Rc::new(RefCell::new(Vec::new()));
    let jmplog = Rc::new(RefCell::new(Vec::new()));

    let mem_write_hook = {
        let writelog = writelog.clone();
        let callback = move |uc: &unicorn::Unicorn,
                             _memtype: unicorn::MemType,
                             addr: u64,
                             size: usize,
                             val: i64| {
            let mut wmut = writelog.borrow_mut();
            let pc = read_pc(uc).unwrap();
            let write_record = WriteRecord {
                pc,
                dest_addr: addr,
                value: val as u64,
                size,
            };
            wmut.push(write_record);
            true
        };
        emu.hook_writeable_mem(callback)
    };

    let visit_hook = {
        let visitor = visitor.clone();
        let callback = move |uc: &unicorn::Unicorn, addr: u64, size: u32| {
            let mut vmut = visitor.borrow_mut();
            let mode = get_mode(&uc);
            let size: usize = (size & 0xF) as usize;
            let registers = uc_general_registers(&uc).unwrap();
            let visit_record = VisitRecord {
                pc: addr,
                mode,
                inst_size: size,
                registers,
            };
            vmut.push(visit_record);
        };
        emu.hook_exec_mem(callback)
    };

    let ret_hook = {
        let retlog = retlog.clone();
        let callback = move |_uc: &unicorn::Unicorn, addr: u64, _size: u32| {
            let mut retlog = retlog.borrow_mut();
            let pc = addr;
            retlog.push(pc);
        };
        emu.hook_rets(callback)
    };

    let indirect_jump_hook = {
        let jmplog = jmplog.clone();
        let callback = move |_uc: &unicorn::Unicorn, addr: u64, _size: u32| {
            let mut jmplog = jmplog.borrow_mut();
            jmplog.push(addr);
        };
        emu.hook_indirect_jumps(callback)
    };

      let _res = emu.start(start_addr, 0, 0, 1024);

    /* Now, clean up the hooks */
    match visit_hook {
        Ok(h) => {
            emu.remove_hook(h).unwrap();
        }
        Err(e) => {
            println!("visit_hook didn't take {:?}", e);
        }
    }
    match mem_write_hook {
        Ok(h) => {
            emu.remove_hook(h).unwrap();
        }
        Err(e) => {
            println!("mem_write_hook didn't take {:?}", e);
        }
    }
    match ret_hook {
        Ok(h) => {
            emu.remove_hook(h).unwrap();
        }
        Err(e) => {
            println!("ret_hook didn't take: {:?}", e);
        }
    }
    match indirect_jump_hook {
        Ok(h) => {
            emu.remove_hook(h).unwrap();
        }
        Err(e) => {
            println!("indirect_jmp_hook didn't take: {:?}", e);
        }
    }

    /* Get the behavioural data from the mutable vectors */
    let registers = emu.read_general_registers().unwrap();
    let vtmp = visitor.clone();
    let visited = vtmp.borrow().to_vec().clone();
    let wtmp = writelog.clone();
    let writelog = wtmp.borrow().to_vec().clone();
    let rtmp = retlog.clone();
    let retlog = rtmp.borrow().to_vec().clone();

    gen::Pod::new(registers, visited, writelog, retlog)
  }
// hatch ends here
