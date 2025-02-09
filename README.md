# range_vec

RangeVec is a data structure for Rust that may have a value for any index, but where only a small range of values are non-default, and only these are stores. It is based on a ring buffer (`VecDeque`), and is useful for applications such as backing storage for scrolling data. It was originally designed for use in change tracking for an emulator's memory viewer.

It requires that the stored type implemnts `Default` and `Eq`, and it will return the default value whenever an index outside of its stored range is accessed. Mutable access is done through closures, so that the ring buffer can be grown or shrunk based on whether the value is equal to the default after being mutated. There may be a guard-based API in the future as well.

## License

This library is licensed under either the Apache License, version 2.0, or the MIT License.
