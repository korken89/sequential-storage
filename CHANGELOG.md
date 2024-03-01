# Changelog

(DD-MM-YY)

## Unreleased

- *Breaking:* Corruption repair is automatic now! The repair functions have been made private.

## 0.9.1 13-02-24

- Added `remove_item` to map

## 0.9.0 11-02-24

- *Breaking:* Storage item key must now also be clone
- Added KeyPointerCache which significantly helps out the map

## 0.8.1 07-02-24

- Added new PagePointerCache that caches more than the PageStateCache. See the readme for more details.

## 0.8.0 05-12-24

- *Breaking:* The item to store is now passed by reference to Map `store_item`
- *Breaking:* Added cache options to the functions to speed up reading the state of the flash.
  To retain the old behaviour you can pass the `NoCache` type as the cache parameter.
- Removed defmt logging since that wasn't being maintained. The format impl for the errors remain.

## 0.7.0 10-01-24

- *Breaking:* Data CRC has been upgraded to 32-bit from 16-bit. Turns out 16-bit has too many collisions.
  This increases the item header size from 6 to 8. The CRC was also moved to the front of the header to
  aid with shutdown/cancellation issues.
- When the state is corrupted, many issues can now be repaired with the repair functions in the map and queue modules
- Made changes to the entire to better survive shutoffs
- *Breaking:* Convert API to async first supporting the traits from embedded-storage-async. Flash
  drivers supporting `sequential-storage` can be wrapped using
  [BlockingAsync](https://docs.embassy.dev/embassy-embedded-hal/git/default/adapter/struct.BlockingAsync.html), and a 
  simple [blocking executor](https://docs.rs/futures/0.3.30/futures/executor/fn.block_on.html) can be used to call the 
  API from a non-async function.

## 0.6.2 - 22-12-23

- Small bug fixes and refactorings including an off-by-one error. Found with added fuzzing from ([#13](https://github.com/tweedegolf/sequential-storage/pull/13))

## 0.6.1 - 16-12-23

- Added queue peek_many and pop_many ([#12](https://github.com/tweedegolf/sequential-storage/pull/12))

## 0.6.0 - 21-11-23

- *Breaking:* Internal overhaul of the code. Both map and queue now use the same `item` module to store and read their data with.
- *Breaking:* Map serialization is no longer done in a stack buffer, but in the buffer provided by the user.
- *Breaking:* Map StorageItemError trait has been removed.
- Added CRC protection of the user data. If user data is corrupted, it will now be skipped as if it wasn't stored.
  If you think it should be reported to the user, let me know and propose an API for that!
- Read word size is no longer required to be 1. It can now be 1-32.

## 0.5.0 - 13-11-23

- *Breaking:* Map `store_item` item no longer uses a ram buffer to temporarily store erased items in.
  Instead it keeps an extra open page so it can copy from one page to another page directly.
  This means the minimum page count for map is now 2.

## 0.4.2 - 13-11-23

- Map no longer erases the flash when corrupted to self-recover. It now just returns an error so the user can choose what to do.

## 0.4.1 - 26-09-23

- Flipped one of the error messages in `queue::pop` and `queue::peek` from `BufferTooBig` to `BufferTooSmall` because that's a lot clearer
- Massive performance bug fixed for the queue. Before it had to read all pages from the start until the first pop or peek data was found.
  Now empty pages are erased which solves this issue.

## 0.4.0 - 04-07-23

- Fixed the queue implementation for devices with a write size of 1
- *Breaking:* The internal storage format for queue has changed, so is incompatible with existing stored memory. The max size has come down to 0x7FFE.

## 0.3.0 - 30-06-23

- Added new queue implementation with `push`, `peek` and `pop` which requires multiwrite flash
- *Breaking:* the map implementation now moved to its own module. You'll need to change your imports.

## 0.2.2 - 11-05-23

- Optimized reading items from flash which reduces the amount of reads by ~30% for small items.

## 0.2.1 - 19-01-23

- Added defmt behind a feature flag. When enabled, the error type implements Format

## 0.2.0 - 13-01-23

- Fixed a scenario where an infinite recursion could lead to a stackoverflow.
  If there's no more space to fit all items, you'll now get an error instead.
- Made the error non-exhaustive so that next time this update wouldn't be a breaking one.

## 0.1.0 - 12-01-23

- Initial release