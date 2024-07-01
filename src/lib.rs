pub mod future {
    use pin_project::pin_project;
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    #[pin_project]
    pub struct ThenFinally<Item, Fut, F, O> {
        #[pin]
        item: Option<Item>,
        #[pin]
        fut: Option<Fut>,
        f: Option<F>,
        output: Option<O>,
    }

    pub trait ThenFinallyFutureExt: Sized {
        /// Consumes the current future into a new one which will execute an asynchronous upon completion of the future
        ///
        /// Note that this will execute the code regardless of a value that the future returns.
        fn then_finally<Fut: Future, F: FnOnce() -> Fut, O>(
            self,
            f: F,
        ) -> ThenFinally<Self, Fut, F, O> {
            ThenFinally {
                item: Some(self),
                fut: None,
                f: Some(f),
                output: None,
            }
        }
    }

    impl<T: Sized> ThenFinallyFutureExt for T {}

    impl<Item: Future<Output = O>, Fut: Future, F, O> Future for ThenFinally<Item, Fut, F, O>
    where
        F: FnOnce() -> Fut,
    {
        type Output = Item::Output;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let mut this = self.project();

            if let Some(item) = this.item.as_mut().as_pin_mut() {
                let output = futures::ready!(item.poll(cx));
                this.output.replace(output);
                let func = this.f.take().expect("function is valid");
                let fut = Some(func());
                this.fut.set(fut);
                this.item.set(None);
            }

            if let Some(fut) = this.fut.as_mut().as_pin_mut() {
                futures::ready!(fut.poll(cx));
                this.fut.set(None);
            }

            let output = this.output.take();

            Poll::Ready(output.expect("output from future to be value"))
        }
    }
}

pub mod stream {
    use futures::{Future, Stream};
    use pin_project::pin_project;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    #[pin_project]
    pub struct Finally<Item, Fut, F> {
        #[pin]
        item: Option<Item>,
        #[pin]
        fut: Option<Fut>,
        f: Option<F>,
    }

    pub trait FinallyStreamExt: Sized {
        /// Consumes the current stream into a new one which will execute an asynchronous upon completion of the stream
        ///
        /// Note that this will execute the code regardless of a value that the stream returns.
        fn finally<Fut: Future, F: FnOnce() -> Fut>(self, f: F) -> Finally<Self, Fut, F> {
            Finally {
                item: Some(self),
                fut: None,
                f: Some(f),
            }
        }
    }

    impl<T: Sized> FinallyStreamExt for T {}

    impl<Item: Stream, Fut: Future, F> Stream for Finally<Item, Fut, F>
    where
        F: FnOnce() -> Fut,
    {
        type Item = Item::Item;
        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let mut this = self.project();

            if let Some(item) = this.item.as_mut().as_pin_mut() {
                match futures::ready!(item.poll_next(cx)) {
                    Some(item) => return Poll::Ready(Some(item)),
                    None => {
                        let func = this.f.take().expect("function is valid");
                        let fut = Some(func());
                        this.fut.set(fut);
                        this.item.set(None);
                    }
                };
            }

            if let Some(fut) = this.fut.as_mut().as_pin_mut() {
                futures::ready!(fut.poll(cx));
                this.fut.set(None);
            }

            Poll::Ready(None)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::future::ThenFinallyFutureExt;
    use crate::stream::FinallyStreamExt;
    use futures::StreamExt;

    #[test]
    fn future_final() {
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

    #[test]
    fn stream_final() {
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
}
