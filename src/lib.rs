#![allow(dead_code)]
/* TODO: Remove this warning once the code is more or less finished.
This is just to prevent useful warnings from being drowned out in
a slew of warnings telling me that code which I haven't gotten around
to using yet is, in fact, unused.
*/
// #![recursion_limit="2048"]
#[macro_use]
extern crate lazy_static;

//extern crate ketos;
// #[macro_use]
//extern crate ketos_derive;

use unicorn;

pub mod emu;

pub mod par;

pub mod gen;
use self::gen::*;

pub mod log;
use self::log::*;

pub mod fit;
use self::fit::*;

pub mod evo;
pub use self::evo::*;
