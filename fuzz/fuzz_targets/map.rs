#![no_main]

use futures::executor::block_on;
use libfuzzer_sys::arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use rand::SeedableRng;
use sequential_storage::{
    cache::{KeyCacheImpl, KeyPointerCache, NoCache, PagePointerCache, PageStateCache},
    map::StorageItem,
    mock_flash::{MockFlashBase, MockFlashError, WriteCountCheck},
    Error,
};
use std::{collections::HashMap, fmt::Debug, ops::Range};

const PAGES: usize = 4;
const WORD_SIZE: usize = 4;
const WORDS_PER_PAGE: usize = 256;

fuzz_target!(|data: Input| match data.cache_type {
    CacheType::NoCache => fuzz(data, NoCache::new()),
    CacheType::PageStateCache => fuzz(data, PageStateCache::<PAGES>::new()),
    CacheType::PagePointerCache => fuzz(data, PagePointerCache::<PAGES>::new()),
    CacheType::KeyPointerCache => fuzz(data, KeyPointerCache::<PAGES, u8, 64>::new()),
});

#[derive(Arbitrary, Debug, Clone)]
struct Input {
    seed: u64,
    fuel: u16,
    ops: Vec<Op>,
    cache_type: CacheType,
}

#[derive(Arbitrary, Debug, Clone)]
enum Op {
    Store(StoreOp),
    Fetch(u8),
    Remove(u8),
}

#[derive(Arbitrary, Debug, Clone)]
struct StoreOp {
    key: u8,
    value_len: u8,
}

impl StoreOp {
    fn into_test_item(self, rng: &mut impl rand::Rng) -> TestItem {
        TestItem {
            key: self.key,
            value: (0..(self.value_len % 8) as usize)
                .map(|_| rng.gen())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TestItem {
    key: u8,
    value: Vec<u8>,
}

impl StorageItem for TestItem {
    type Key = u8;

    type Error = ();

    fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        if buffer.len() < 1 + self.value.len() {
            return Err(());
        }

        buffer[0] = self.key;
        buffer[1..][..self.value.len()].copy_from_slice(&self.value);

        Ok(1 + self.value.len())
    }

    fn deserialize_from(buffer: &[u8]) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        Ok(Self {
            key: buffer[0],
            value: buffer[1..].to_vec(),
        })
    }

    fn key(&self) -> Self::Key {
        self.key
    }

    fn deserialize_key_only(buffer: &[u8]) -> Result<Self::Key, Self::Error>
    where
        Self: Sized,
    {
        Ok(buffer[0])
    }
}

#[derive(Arbitrary, Debug, Clone)]
enum CacheType {
    NoCache,
    PageStateCache,
    PagePointerCache,
    KeyPointerCache,
}

fn fuzz(ops: Input, mut cache: impl KeyCacheImpl<u8> + Debug) {
    let mut flash = MockFlashBase::<PAGES, WORD_SIZE, WORDS_PER_PAGE>::new(
        if ops.ops.iter().any(|op| matches!(op, Op::Remove(_))) {
            WriteCountCheck::Twice
        } else {
            WriteCountCheck::OnceOnly
        },
        Some(ops.fuel as u32),
        true,
    );
    const FLASH_RANGE: Range<u32> = 0x000..0x1000;

    let mut map = HashMap::new();
    #[repr(align(4))]
    struct AlignedBuf([u8; 260]);
    let mut buf = AlignedBuf([0; 260]); // Max length of test item serialized, rounded up to align to flash word.

    let mut rng = rand_pcg::Pcg32::seed_from_u64(ops.seed);

    #[cfg(fuzzing_repro)]
    eprintln!("\n=== START ===\n");

    for op in ops.ops.into_iter() {
        #[cfg(fuzzing_repro)]
        eprintln!("{}", flash.print_items());
        #[cfg(fuzzing_repro)]
        eprintln!("{:?}", cache);
        #[cfg(fuzzing_repro)]
        eprintln!("=== OP: {op:?} ===");

        match op.clone() {
            Op::Store(op) => {
                let item = op.into_test_item(&mut rng);
                match block_on(sequential_storage::map::store_item(
                    &mut flash,
                    FLASH_RANGE,
                    &mut cache,
                    &mut buf.0,
                    &item,
                )) {
                    Ok(_) => {
                        map.insert(item.key, item.value);
                    }
                    Err(Error::FullStorage) => {}
                    Err(Error::Storage {
                        value: MockFlashError::EarlyShutoff(_),
                        backtrace: _backtrace,
                    }) => {
                        match block_on(sequential_storage::map::fetch_item::<TestItem, _>(
                            &mut flash,
                            FLASH_RANGE,
                            &mut cache,
                            &mut buf.0,
                            item.key,
                        )) {
                            Ok(Some(check_item))
                                if check_item.key == item.key && check_item.value == item.value =>
                            {
                                #[cfg(fuzzing_repro)]
                                eprintln!("Early shutoff when storing {item:?}! (but it still stored fully). Originated from:\n{_backtrace:#}");
                                // Even though we got a shutoff, it still managed to store well
                                map.insert(item.key, item.value);
                            }
                            _ => {
                                // Could not fetch the item we stored...
                                #[cfg(fuzzing_repro)]
                                eprintln!("Early shutoff when storing {item:?}! Originated from:\n{_backtrace:#}");
                            }
                        }
                    }
                    Err(Error::Corrupted {
                        backtrace: _backtrace,
                    }) => {
                        #[cfg(fuzzing_repro)]
                        eprintln!("Corrupted when storing! Originated from:\n{_backtrace:#}");
                        panic!("Corrupted!");
                    }
                    Err(e) => panic!("{e:?}"),
                }
            }
            Op::Fetch(key) => {
                match block_on(sequential_storage::map::fetch_item::<TestItem, _>(
                    &mut flash,
                    FLASH_RANGE,
                    &mut cache,
                    &mut buf.0,
                    key,
                )) {
                    Ok(Some(fetch_result)) => {
                        let map_value = map
                            .get(&key)
                            .expect(&format!("Map doesn't contain: {fetch_result:?}"));
                        assert_eq!(key, fetch_result.key, "Mismatching keys");
                        assert_eq!(map_value, &fetch_result.value, "Mismatching values");
                    }
                    Ok(None) => {
                        assert_eq!(None, map.get(&key));
                    }
                    Err(Error::Storage {
                        value: MockFlashError::EarlyShutoff(_),
                        backtrace: _backtrace,
                    }) => {
                        #[cfg(fuzzing_repro)]
                        eprintln!("Early shutoff when fetching! Originated from:\n{_backtrace:#}");
                    }
                    Err(Error::Corrupted {
                        backtrace: _backtrace,
                    }) => {
                        #[cfg(fuzzing_repro)]
                        eprintln!("Corrupted when fetching! Originated from:\n{_backtrace:#}");
                        panic!("Corrupted!");
                    }
                    Err(e) => panic!("{e:#?}"),
                }
            }
            Op::Remove(key) => {
                match block_on(sequential_storage::map::remove_item::<TestItem, _>(
                    &mut flash,
                    FLASH_RANGE,
                    &mut cache,
                    &mut buf.0,
                    key,
                )) {
                    Ok(()) => {
                        map.remove(&key);
                    }
                    Err(Error::Storage {
                        value: MockFlashError::EarlyShutoff(_),
                        backtrace: _backtrace,
                    }) => {
                        match block_on(sequential_storage::map::fetch_item::<TestItem, _>(
                            &mut flash,
                            FLASH_RANGE,
                            &mut cache,
                            &mut buf.0,
                            key,
                        )) {
                            Ok(Some(_)) => {
                                #[cfg(fuzzing_repro)]
                                eprintln!("Early shutoff when removing item {key}! Originated from:\n{_backtrace:#}");
                            }
                            _ => {
                                // Could not fetch the item we stored...
                                #[cfg(fuzzing_repro)]
                                eprintln!("Early shutoff when removing item {key}! (but it still removed fully). Originated from:\n{_backtrace:#}");
                                // Even though we got a shutoff, it still managed to store well
                                map.remove(&key);
                            }
                        }
                    }
                    Err(Error::Corrupted {
                        backtrace: _backtrace,
                    }) => {
                        #[cfg(fuzzing_repro)]
                        eprintln!("Corrupted when removing! Originated from:\n{_backtrace:#}");
                        panic!("Corrupted!");
                    }
                    Err(e) => panic!("{e:?}"),
                }
            }
        }
    }
}