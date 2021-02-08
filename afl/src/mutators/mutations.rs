use crate::{
    corpus::InMemoryCorpus,
    inputs::{BytesInput, HasBytesVec, Input},
    mutators::Corpus,
    mutators::*,
    state::State,
    utils::Rand,
    AflError,
};

use alloc::{borrow::ToOwned, vec::Vec};

use std::cmp::{max, min};
#[cfg(feature = "std")]
use std::{
    fs::File,
    io::{BufRead, BufReader},
};

const ARITH_MAX: u64 = 35;

const INTERESTING_8: [i8; 9] = [-128, -1, 0, 1, 16, 32, 64, 100, 127];
const INTERESTING_16: [i16; 19] = [
    -128, -1, 0, 1, 16, 32, 64, 100, 127, -32768, -129, 128, 255, 256, 512, 1000, 1024, 4096, 32767,
];
const INTERESTING_32: [i32; 27] = [
    -128,
    -1,
    0,
    1,
    16,
    32,
    64,
    100,
    127,
    -32768,
    -129,
    128,
    255,
    256,
    512,
    1000,
    1024,
    4096,
    32767,
    -2147483648,
    -100663046,
    -32769,
    32768,
    65535,
    65536,
    100663045,
    2147483647,
];

/// The result of a mutation.
/// If the mutation got skipped, the target
/// will not be executed with the returned input.
#[derive(Clone, Copy, Debug)]
pub enum MutationResult {
    Mutated,
    Skipped,
}

// TODO maybe the mutator arg is not needed
/// The generic function type that identifies mutations
pub type MutationFunction<I, M, R, S> =
    fn(&mut M, &mut R, &mut S, &mut I) -> Result<MutationResult, AflError>;

pub trait ComposedByMutations<C, I, R, S>
where
    C: Corpus<I, R>,
    I: Input,
    R: Rand,
    S: HasCorpus<C> + HasMetadata,
{
    /// Get a mutation by index
    fn mutation_by_idx(&self, index: usize) -> MutationFunction<I, Self, R, S>;

    /// Get the number of mutations
    fn mutations_count(&self) -> usize;

    /// Add a mutation
    fn add_mutation(&mut self, mutation: MutationFunction<I, Self, R, S>);
}

#[inline]
fn self_mem_move(data: &mut [u8], from: usize, to: usize, len: usize) {
    debug_assert!(from + len <= data.len());
    debug_assert!(to + len <= data.len());
    let ptr = data.as_mut_ptr();
    unsafe { core::ptr::copy(ptr.offset(from as isize), ptr.offset(to as isize), len) }
}

#[inline]
fn mem_move(dst: &mut [u8], src: &[u8], from: usize, to: usize, len: usize) {
    debug_assert!(from + len <= src.len());
    debug_assert!(to + len <= dst.len());
    let dst_ptr = dst.as_mut_ptr();
    let src_ptr = src.as_ptr();
    unsafe {
        core::ptr::copy(
            src_ptr.offset(from as isize),
            dst_ptr.offset(to as isize),
            len,
        )
    }
}

#[inline]
fn mem_set(data: &mut [u8], from: usize, len: usize, val: u8) {
    debug_assert!(from + len <= data.len());
    let ptr = data.as_mut_ptr();
    unsafe { core::ptr::write_bytes(ptr.offset(from as isize), val, len) }
}

/// Bitflip mutation for inputs with a bytes vector
pub fn mutation_bitflip<I, M, R, S>(
    _: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    I: Input + HasBytesVec,
    R: Rand,
{
    if input.bytes().len() == 0 {
        Ok(MutationResult::Skipped)
    } else {
        let bit = rand.below((input.bytes().len() << 3) as u64) as usize;
        unsafe {
            // moar speed, no bound check
            *input.bytes_mut().get_unchecked_mut(bit >> 3) ^= (128 >> (bit & 7)) as u8;
        }
        Ok(MutationResult::Mutated)
    }
}

pub fn mutation_byteflip<I, M, R, S>(
    _: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    I: Input + HasBytesVec,
    R: Rand,
{
    if input.bytes().len() == 0 {
        Ok(MutationResult::Skipped)
    } else {
        let idx = rand.below(input.bytes().len() as u64) as usize;
        unsafe {
            // moar speed, no bound check
            *input.bytes_mut().get_unchecked_mut(idx) ^= 0xff;
        }
        Ok(MutationResult::Mutated)
    }
}

pub fn mutation_byteinc<I, M, R, S>(
    _: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    I: Input + HasBytesVec,
    R: Rand,
{
    if input.bytes().len() == 0 {
        Ok(MutationResult::Skipped)
    } else {
        let idx = rand.below(input.bytes().len() as u64) as usize;
        unsafe {
            // moar speed, no bound check
            let ptr = input.bytes_mut().get_unchecked_mut(idx);
            *ptr = (*ptr).wrapping_add(1);
        }
        Ok(MutationResult::Mutated)
    }
}

pub fn mutation_bytedec<I, M, R, S>(
    _: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    I: Input + HasBytesVec,
    R: Rand,
{
    if input.bytes().len() == 0 {
        Ok(MutationResult::Skipped)
    } else {
        let idx = rand.below(input.bytes().len() as u64) as usize;
        unsafe {
            // moar speed, no bound check
            let ptr = input.bytes_mut().get_unchecked_mut(idx);
            *ptr = (*ptr).wrapping_sub(1);
        }
        Ok(MutationResult::Mutated)
    }
}

pub fn mutation_byteneg<I, M, R, S>(
    _: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    I: Input + HasBytesVec,
    R: Rand,
{
    if input.bytes().len() == 0 {
        Ok(MutationResult::Skipped)
    } else {
        let idx = rand.below(input.bytes().len() as u64) as usize;
        unsafe {
            // moar speed, no bound check
            *input.bytes_mut().get_unchecked_mut(idx) = !(*input.bytes().get_unchecked(idx));
        }
        Ok(MutationResult::Mutated)
    }
}

pub fn mutation_byterand<I, M, R, S>(
    _: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    I: Input + HasBytesVec,
    R: Rand,
{
    if input.bytes().len() == 0 {
        Ok(MutationResult::Skipped)
    } else {
        let idx = rand.below(input.bytes().len() as u64) as usize;
        unsafe {
            // moar speed, no bound check
            *input.bytes_mut().get_unchecked_mut(idx) = rand.below(256) as u8;
        }
        Ok(MutationResult::Mutated)
    }
}

pub fn mutation_byteadd<I, M, R, S>(
    _: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    I: Input + HasBytesVec,
    R: Rand,
{
    if input.bytes().len() <= 1 {
        Ok(MutationResult::Skipped)
    } else {
        let idx = rand.below(input.bytes().len() as u64 - 1) as usize;
        unsafe {
            // moar speed, no bound check
            let ptr = input.bytes_mut().get_unchecked_mut(idx) as *mut u8;
            let num = 1 + rand.below(ARITH_MAX) as u8;
            match rand.below(2) {
                0 => *ptr = (*ptr).wrapping_add(num),
                _ => *ptr = (*ptr).wrapping_sub(num),
            };
        }
        Ok(MutationResult::Mutated)
    }
}

pub fn mutation_wordadd<I, M, R, S>(
    _: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    I: Input + HasBytesVec,
    R: Rand,
{
    if input.bytes().len() <= 1 {
        Ok(MutationResult::Skipped)
    } else {
        let idx = rand.below(input.bytes().len() as u64 - 1) as usize;
        unsafe {
            // moar speed, no bound check
            let ptr = input.bytes_mut().get_unchecked_mut(idx) as *mut _ as *mut u16;
            let num = 1 + rand.below(ARITH_MAX) as u16;
            match rand.below(4) {
                0 => *ptr = (*ptr).wrapping_add(num),
                1 => *ptr = (*ptr).wrapping_sub(num),
                2 => *ptr = ((*ptr).swap_bytes().wrapping_add(num)).swap_bytes(),
                _ => *ptr = ((*ptr).swap_bytes().wrapping_sub(num)).swap_bytes(),
            };
        }
        Ok(MutationResult::Mutated)
    }
}

pub fn mutation_dwordadd<I, M, R, S>(
    _: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    I: Input + HasBytesVec,
    R: Rand,
{
    if input.bytes().len() <= 1 {
        Ok(MutationResult::Skipped)
    } else {
        let idx = rand.below(input.bytes().len() as u64 - 1) as usize;
        unsafe {
            // moar speed, no bound check
            let ptr = input.bytes_mut().get_unchecked_mut(idx) as *mut _ as *mut u32;
            let num = 1 + rand.below(ARITH_MAX) as u32;
            match rand.below(4) {
                0 => *ptr = (*ptr).wrapping_add(num),
                1 => *ptr = (*ptr).wrapping_sub(num),
                2 => *ptr = ((*ptr).swap_bytes().wrapping_add(num)).swap_bytes(),
                _ => *ptr = ((*ptr).swap_bytes().wrapping_sub(num)).swap_bytes(),
            };
        }
        Ok(MutationResult::Mutated)
    }
}

pub fn mutation_qwordadd<I, M, R, S>(
    _: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    I: Input + HasBytesVec,
    R: Rand,
{
    if input.bytes().len() <= 1 {
        Ok(MutationResult::Skipped)
    } else {
        let idx = rand.below(input.bytes().len() as u64 - 1) as usize;
        unsafe {
            // moar speed, no bound check
            let ptr = input.bytes_mut().get_unchecked_mut(idx) as *mut _ as *mut u64;
            let num = 1 + rand.below(ARITH_MAX) as u64;
            match rand.below(4) {
                0 => *ptr += num,
                1 => *ptr -= num,
                2 => *ptr = ((*ptr).swap_bytes() + num).swap_bytes(),
                _ => *ptr = ((*ptr).swap_bytes() - num).swap_bytes(),
            };
        }
        Ok(MutationResult::Mutated)
    }
}

pub fn mutation_byteinteresting<I, M, R, S>(
    _: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    I: Input + HasBytesVec,
    R: Rand,
{
    if input.bytes().len() <= 1 {
        Ok(MutationResult::Skipped)
    } else {
        let idx = rand.below(input.bytes().len() as u64 - 1) as usize;
        let val = INTERESTING_8[rand.below(INTERESTING_8.len() as u64) as usize] as u8;
        unsafe {
            // moar speed, no bound check
            *input.bytes_mut().get_unchecked_mut(idx) = val;
        }
        Ok(MutationResult::Mutated)
    }
}

pub fn mutation_wordinteresting<I, M, R, S>(
    _: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    I: Input + HasBytesVec,
    R: Rand,
{
    if input.bytes().len() <= 1 {
        Ok(MutationResult::Skipped)
    } else {
        let idx = rand.below(input.bytes().len() as u64 - 1) as usize;
        let val = INTERESTING_16[rand.below(INTERESTING_8.len() as u64) as usize] as u16;
        unsafe {
            // moar speed, no bound check
            let ptr = input.bytes_mut().get_unchecked_mut(idx) as *mut _ as *mut u16;
            if rand.below(2) == 0 {
                *ptr = val;
            } else {
                *ptr = val.swap_bytes();
            }
        }
        Ok(MutationResult::Mutated)
    }
}

pub fn mutation_dwordinteresting<I, M, R, S>(
    _: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    I: Input + HasBytesVec,
    R: Rand,
{
    if input.bytes().len() <= 1 {
        Ok(MutationResult::Skipped)
    } else {
        let idx = rand.below(input.bytes().len() as u64 - 1) as usize;
        let val = INTERESTING_32[rand.below(INTERESTING_8.len() as u64) as usize] as u32;
        unsafe {
            // moar speed, no bound check
            let ptr = input.bytes_mut().get_unchecked_mut(idx) as *mut _ as *mut u32;
            if rand.below(2) == 0 {
                *ptr = val;
            } else {
                *ptr = val.swap_bytes();
            }
        }
        Ok(MutationResult::Mutated)
    }
}

pub fn mutation_bytesdelete<I, M, R, S>(
    _: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    I: Input + HasBytesVec,
    R: Rand,
{
    let size = input.bytes().len();
    if size <= 2 {
        return Ok(MutationResult::Skipped);
    }

    let off = rand.below(size as u64) as usize;
    let len = rand.below((size - off) as u64) as usize;
    input.bytes_mut().drain(off..off + len);

    Ok(MutationResult::Mutated)
}

pub fn mutation_bytesexpand<I, M, R, S>(
    // TODO: max_size instead of mutator?
    mutator: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    M: HasMaxSize,
    I: Input + HasBytesVec,
    R: Rand,
{
    let size = input.bytes().len();
    if size == 0 {
        return Ok(MutationResult::Skipped);
    }
    let off = rand.below(size as u64 - 1) as usize;
    let len = rand.below(min(16, mutator.max_size() as u64 - 1) + 1) as usize;

    input.bytes_mut().resize(size + len, 0);
    //println!("Size {}, off {}, len {}", size, off, len);
    self_mem_move(input.bytes_mut(), 0, 0 + off, len);
    Ok(MutationResult::Mutated)
}

/* TODO
/// Insert a dictionary token
pub fn mutation_tokeninsert<I, M, R, S>(
    mutator: &mut M,
    rand: &mut R,
    state: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    M: HasMaxSize,
    I: Input + HasBytesVec,
    R: Rand,
    S: HasMetadata,
{
    let tokens: Vec<Vec<u8>> = state.metadata().get().unwrap();
    if mutator.tokens.size() == 0 {
        return Ok(MutationResult::Skipped);
    }
    let token = &mutator.tokens[rand.below(token.size())];
    let token_len = token.size();
    let size = input.bytes().len();
    let off = if size == 0 {
        0
    } else {
        rand.below(core::cmp::min(
            size,
            (mutator.max_size() - token_len) as u64,
        )) as usize
    } as usize;

    input.bytes_mut().resize(size + token_len, 0);
    mem_move(input.bytes_mut(), token, 0, off, len);
    Ok(MutationResult::Mutated)
}

/// Overwrite with a dictionary token
pub fn mutation_tokenreplace<I, M, R, S>(
    mutator: &mut M,
    rand: &mut R,
    state: &S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    M: HasMaxSize,
    I: Input + HasBytesVec,
    R: Rand,
    S: HasMetadata,
{
    if mutator.tokens.size() > len || !len {
        return Ok(MutationResult::Skipped);
    }
    let token = &mutator.tokens[rand.below(token.size())];
    let token_len = token.size();
    let size = input.bytes().len();
    let off = rand.below((mutator.max_size() - token_len) as u64) as usize;
    mem_move(input.bytes_mut(), token, 0, off, len);
    Ok(MutationResult::Mutated)
}
*/

pub fn mutation_bytesinsert<I, M, R, S>(
    mutator: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    M: HasMaxSize,
    I: Input + HasBytesVec,
    R: Rand,
{
    let mut size = input.bytes().len();
    if size == 0 {
        // Generate random bytes if we're empty.
        input
            .bytes_mut()
            .append(&mut rand.next().to_be_bytes().to_vec());
        size = input.bytes().len();
    }
    let off = rand.below(size as u64 - 1) as usize;
    let len = rand.below(core::cmp::min(16, mutator.max_size() as u64)) as usize;

    let val = input.bytes()[rand.below(size as u64) as usize];
    input.bytes_mut().resize(off + (2 * len), 0);
    self_mem_move(input.bytes_mut(), off, off + len, len);
    mem_set(input.bytes_mut(), off, len, val);
    Ok(MutationResult::Mutated)
}

pub fn mutation_bytesrandinsert<I, M, R, S>(
    mutator: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    M: HasMaxSize,
    I: Input + HasBytesVec,
    R: Rand,
{
    let size = input.bytes().len();
    let off = if size == 0 {
        0
    } else {
        rand.below(size as u64 - 1)
    } as usize;
    let len = rand.below(core::cmp::min(16, mutator.max_size() as u64)) as usize;

    let val = rand.below(256) as u8;
    input.bytes_mut().resize(off + (2 * len), 0);
    self_mem_move(input.bytes_mut(), off, off + len, len);
    mem_set(input.bytes_mut(), off, len, val);
    Ok(MutationResult::Mutated)
}

pub fn mutation_bytesset<I, M, R, S>(
    _: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    I: Input + HasBytesVec,
    R: Rand,
{
    let size = input.bytes().len();
    if size == 0 {
        return Ok(MutationResult::Skipped);
    }

    let val = input.bytes()[rand.below(size as u64) as usize];
    let start = if size == 1 {
        0
    } else {
        rand.below(size as u64 - 1) as usize
    };
    let end = rand.between(start as u64, size as u64) as usize;
    mem_set(input.bytes_mut(), start, end - start, val);
    Ok(MutationResult::Mutated)
}

pub fn mutation_bytesrandset<I, M, R, S>(
    _: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    I: Input + HasBytesVec,
    R: Rand,
{
    let size = input.bytes().len();
    if size == 0 {
        return Ok(MutationResult::Skipped);
    }

    let val = rand.below(256) as u8;
    let start = if size == 1 {
        0
    } else {
        rand.below(size as u64 - 1) as usize
    };
    let len = rand.below((size - start) as u64) as usize;
    mem_set(input.bytes_mut(), start, len, val);
    Ok(MutationResult::Mutated)
}

pub fn mutation_bytescopy<I, M, R, S>(
    _: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    I: Input + HasBytesVec,
    R: Rand,
{
    let size = input.bytes().len();
    if size <= 1 {
        return Ok(MutationResult::Skipped);
    }

    let from = rand.below(input.bytes().len() as u64 - 1) as usize;
    let to = rand.below(input.bytes().len() as u64 - 1) as usize;
    let len = rand.below((size - core::cmp::max(from, to)) as u64) as usize;

    self_mem_move(input.bytes_mut(), from, to, len);
    Ok(MutationResult::Mutated)
}

pub fn mutation_bytesswap<I, M, R, S>(
    _: &mut M,
    rand: &mut R,
    _: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    I: Input + HasBytesVec,
    R: Rand,
{
    let size = input.bytes().len();
    if size <= 1 {
        return Ok(MutationResult::Skipped);
    }

    let first = rand.below(input.bytes().len() as u64 - 1) as usize;
    let second = rand.below(input.bytes().len() as u64 - 1) as usize;
    let len = rand.below((size - max(first, second)) as u64) as usize;

    let tmp = input.bytes()[first..len].to_vec();
    self_mem_move(input.bytes_mut(), second, first, len);
    mem_move(input.bytes_mut(), &tmp, 0, second, len);
    Ok(MutationResult::Mutated)
}

/// Returns the first and last diff position between the given vectors, stopping at the min len
fn locate_diffs(this: &[u8], other: &[u8]) -> (i64, i64) {
    let mut first_diff: i64 = -1;
    let mut last_diff: i64 = -1;
    for (i, (this_el, other_el)) in this.iter().zip(other.iter()).enumerate() {
        if this_el != other_el {
            if first_diff < 0 {
                first_diff = i as i64;
            }
            last_diff = i as i64;
        }
    }

    (first_diff, last_diff)
}

/// Splicing mutator
pub fn mutation_splice<C, I, M, R, S>(
    _: &mut M,
    rand: &mut R,
    state: &mut S,
    input: &mut I,
) -> Result<MutationResult, AflError>
where
    C: Corpus<I, R>,
    I: Input + HasBytesVec,
    R: Rand,
    S: HasCorpus<C>,
{
    // We don't want to use the testcase we're already using for splicing
    let (other_testcase, idx) = state.corpus().random_entry(rand)?;
    if idx == state.corpus().current_testcase().1 {
        return Ok(MutationResult::Skipped);
    }
    // println!("Input: {:?}, other input: {:?}", input.bytes(), other.bytes());
    let mut other_ref = other_testcase.borrow_mut();
    let other = other_ref.load_input()?;

    let mut counter = 0;
    let (first_diff, last_diff) = loop {
        let (f, l) = locate_diffs(input.bytes(), other.bytes());
        // println!("Diffs were between {} and {}", f, l);
        if f != l && f >= 0 && l >= 2 {
            break (f, l);
        }
        if counter == 3 {
            return Ok(MutationResult::Skipped);
        }
        counter += 1;
    };

    let split_at = rand.between(first_diff as u64, last_diff as u64) as usize;

    // println!("Splicing at {}", split_at);

    input
        .bytes_mut()
        .splice(split_at.., other.bytes()[split_at..].iter().cloned());

    // println!("Splice result: {:?}, input is now: {:?}", split_result, input.bytes());

    Ok(MutationResult::Mutated)
}

// Converts a hex u8 to its u8 value: 'A' -> 10 etc.
fn from_hex(hex: u8) -> Result<u8, AflError> {
    if hex >= 48 && hex <= 57 {
        return Ok(hex - 48);
    }
    if hex >= 65 && hex <= 70 {
        return Ok(hex - 55);
    }
    if hex >= 97 && hex <= 102 {
        return Ok(hex - 87);
    }
    return Err(AflError::IllegalArgument("".to_owned()));
}

/// Decodes a dictionary token: 'foo\x41\\and\"bar' -> 'fooA\and"bar'
pub fn str_decode(item: &str) -> Result<Vec<u8>, AflError> {
    let mut token: Vec<u8> = Vec::new();
    let item: Vec<u8> = item.as_bytes().to_vec();
    let backslash: u8 = 92; // '\\'
    let mut take_next: bool = false;
    let mut take_next_two: u32 = 0;
    let mut decoded: u8 = 0;

    for c in item {
        if take_next_two == 1 {
            decoded = from_hex(c)? << 4;
            take_next_two = 2;
        } else if take_next_two == 2 {
            decoded += from_hex(c)?;
            token.push(decoded);
            take_next_two = 0;
        } else {
            if c != backslash || take_next {
                if take_next && (c == 120 || c == 88) {
                    take_next_two = 1;
                } else {
                    token.push(c);
                }
                take_next = false;
            } else {
                take_next = true;
            }
        }
    }

    return Ok(token);
}

/// Adds a token to a dictionary, checking it is not a duplicate
pub fn add_token(tokens: &mut Vec<Vec<u8>>, token: &Vec<u8>) -> u32 {
    if tokens.contains(token) {
        return 0;
    }
    tokens.push(token.to_vec());
    return 1;
}

/// Read a dictionary file and return the number of entries read
#[cfg(feature = "std")]
pub fn read_tokens_file(f: &str, tokens: &mut Vec<Vec<u8>>) -> Result<u32, AflError> {
    let mut entries = 0;

    println!("Loading tokens file {:?} ...", &f);

    let file = File::open(&f)?; // panic if not found
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line.unwrap();
        let line = line.trim_start().trim_end();

        // we are only interested in '"..."', not prefixed 'foo = '
        let start = line.chars().nth(0);
        if line.len() == 0 || start == Some('#') {
            continue;
        }
        let pos_quote = match line.find("\"") {
            Some(x) => x,
            _ => {
                return Err(AflError::IllegalArgument(
                    "Illegal line: ".to_owned() + line,
                ))
            }
        };
        if line.chars().nth(line.len() - 1) != Some('"') {
            return Err(AflError::IllegalArgument(
                "Illegal line: ".to_owned() + line,
            ));
        }

        // extract item
        let item = match line.get(pos_quote + 1..line.len() - 1) {
            Some(x) => x,
            _ => {
                return Err(AflError::IllegalArgument(
                    "Illegal line: ".to_owned() + line,
                ))
            }
        };
        if item.len() == 0 {
            continue;
        }

        // decode
        let token: Vec<u8> = match str_decode(item) {
            Ok(val) => val,
            Err(_) => {
                return Err(AflError::IllegalArgument(
                    "Illegal line (hex decoding): ".to_owned() + line,
                ))
            }
        };

        // add
        entries += add_token(tokens, &token);
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "std")]
    use std::fs;

    #[cfg(feature = "std")]
    use crate::mutators::read_tokens_file;

    use super::*;
    use crate::{
        corpus::{Corpus, InMemoryCorpus},
        executors::InMemoryExecutor,
        inputs::BytesInput,
        state::State,
        utils::StdRand,
    };

    #[cfg(feature = "std")]
    #[test]
    fn test_read_tokens() {
        let _ = fs::remove_file("test.tkns");
        let data = r###"
# comment
token1@123="AAA"
token1="A\x41A"
"A\AA"
token2="B"
        "###;
        fs::write("test.tkns", data).expect("Unable to write test.tkns");
        let mut v: Vec<Vec<u8>> = Vec::new();
        let res = read_tokens_file(&"test.tkns".to_string(), &mut v).unwrap();
        #[cfg(feature = "std")]
        println!("Token file entries: {:?}", res);
        assert_eq!(res, 2);
        let _ = fs::remove_file("test.tkns");
    }

    struct WithMaxSize {}
    impl HasMaxSize for WithMaxSize {
        fn max_size(&self) -> usize {
            16000 as usize
        }

        fn set_max_size(&mut self, max_size: usize) {
            todo!("Not needed");
        }
    }

    #[test]
    fn test_mutators() {
        let mut inputs = vec![
            BytesInput::new(vec![0x13, 0x37]),
            BytesInput::new(vec![0xFF; 2048]),
            BytesInput::new(vec![0xFF; 50000]),
            BytesInput::new(vec![0x0]),
            BytesInput::new(vec![]),
        ];

        let mut mutator = WithMaxSize {};

        let mut rand = StdRand::new(1337);
        let mut corpus: InMemoryCorpus<_, StdRand> = InMemoryCorpus::new();

        corpus.add(BytesInput::new(vec![0x42; 0x1337]).into());

        let mut state = State::new(corpus, ());

        let mut mutations: Vec<MutationFunction<BytesInput, WithMaxSize, StdRand, _>> = vec![];

        mutations.push(mutation_bitflip);
        mutations.push(mutation_byteflip);
        mutations.push(mutation_byteinc);
        mutations.push(mutation_bytedec);
        mutations.push(mutation_byteneg);
        mutations.push(mutation_byterand);

        mutations.push(mutation_byteadd);
        mutations.push(mutation_wordadd);
        mutations.push(mutation_dwordadd);
        mutations.push(mutation_qwordadd);
        mutations.push(mutation_byteinteresting);
        mutations.push(mutation_wordinteresting);
        mutations.push(mutation_dwordinteresting);

        mutations.push(mutation_bytesdelete);
        mutations.push(mutation_bytesdelete);
        mutations.push(mutation_bytesdelete);
        mutations.push(mutation_bytesdelete);
        mutations.push(mutation_bytesexpand);
        mutations.push(mutation_bytesinsert);
        mutations.push(mutation_bytesrandinsert);
        mutations.push(mutation_bytesset);
        mutations.push(mutation_bytesrandset);
        mutations.push(mutation_bytescopy);
        mutations.push(mutation_bytesswap);

        for _ in 0..3 {
            let mut new_testcases = vec![];
            for mutation in &mutations {
                for input in inputs.iter() {
                    let mut mutant = input.clone();
                    match mutation(&mut mutator, &mut rand, &mut state, &mut mutant).unwrap() {
                        MutationResult::Mutated => new_testcases.push(mutant),
                        MutationResult::Skipped => (),
                    };
                }
            }
            inputs.append(&mut new_testcases);
        }

        /* TODO
        scheduled.add_mutation(mutation_tokeninsert);
        scheduled.add_mutation(mutation_tokenreplace);
        */
    }
}
