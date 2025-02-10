# range_vec
[![Latest version](https://img.shields.io/crates/v/range_vec)](https://crates.io/crates/range_vec)
[![Documentation](https://docs.rs/range_vec/badge.svg)](https://docs.rs/range_vec)
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/dacid44/range_vec/blob/main/LICENSE-MIT)
[![Apache](https://img.shields.io/badge/license-Apache-blue.svg)](https://github.com/dacid44/range_vec/blob/main/LICENSE-APACHE)


`RangeVec` is a data structure for Rust that may have a value for any index, but where only a small range of values are non-default, and only these are stored. It is based on a ring buffer (`VecDeque`) so that it may efficiently grow in either direction. It is useful for applications such as backing storage for scrolling data, and was originally designed for use in change tracking for an emulator's memory viewer.

`RangeVec` requires that the stored type implement `Default` and `Eq`, and it will return the default value whenever an index outside of its stored range is accessed. The stored range will automatically be grown or shrunk to exactly match the smallest possible range of non-default values after every mutation. To facilitate this, all mutable access is currently done through closures, so that the ring buffer may be adjusted based on whether the value is equal to the default after mutation. There may be a guard-based API in the future as well.

## License

This library is licensed under either the Apache License, version 2.0, or the MIT License.
