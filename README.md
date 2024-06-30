# futures-finally

A small utility that provides two functions to executes a future after the completion of a future or stream.

## Motivation

I written this small library due to wanting to execute specific asynchronous code after the completion of a future or stream. This could be awaiting another future, sending data through a channel, etc.

## Example

### Future example

```rust
use futures_finally::future::ThenFinallyFutureExt;

fn main() {
    futures::executor::block_on(async move {
        let mut val = 0;

        futures::future::ready(())
            .then_finally(|| async {
                val = 1;
            })
            .await;

        assert_eq!(val, 1);
    });
}
```

### Stream example

```rust
use futures::StreamExt;
use futures_finally::stream::FinallyStreamExt;

fn main() {
    futures::executor::block_on(async move {
        let mut val = 0;

        let st = futures::stream::once(async { 0 }).finally(|| async {
            val = 1;
        });

        futures::pin_mut!(st);

        while let Some(v) = st.next().await {
            assert_eq!(v, 0);
        }

        assert_eq!(val, 1);
    });
}
```

## MSRV

The minimum supported rust version is 1.75, which can be changed in the future. There is no guarantee that this library will work on older versions of rust.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.